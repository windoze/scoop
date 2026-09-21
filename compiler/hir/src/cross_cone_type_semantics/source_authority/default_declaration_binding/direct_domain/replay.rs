use super::*;
use PersistentAccessConstraintV1 as Constraint;

pub(super) struct Replay<'a, 'f> {
    foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    pub meter: &'a mut BudgetMeter,
    path: &'a WirePath,
    persistent: Vec<Constraint>,
    generic: Vec<PersistentGenericTypeId>,
}
pub(super) fn query(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), DomainError> {
    let count = foundation.source().entries().sources.records().len();
    meter.charge_work((u64::from(count.max(1).ilog2()) + 1) * 65, path)?;
    Ok(())
}
impl<'a, 'f> Replay<'a, 'f> {
    pub fn new(
        foundation: &'a BoundTypeFoundationSourcesV1<'f>,
        count: usize,
        meter: &'a mut BudgetMeter,
        path: &'a WirePath,
    ) -> Result<Self, DomainError> {
        meter.check_semantic_depth(count as u64, path)?;
        meter.check_table_entries((count as u64).saturating_mul(2), path)?;
        let mut persistent = Vec::new();
        let mut generic = Vec::new();
        meter.charge_owned_bytes(
            (count as u64).saturating_mul(
                2 * std::mem::size_of::<Constraint>() as u64
                    + std::mem::size_of::<PersistentGenericTypeId>() as u64,
            ),
            path,
        )?;
        meter.try_reserve_exact(
            &mut persistent,
            (count as u64).saturating_mul(2),
            std::mem::size_of::<Constraint>() as u64,
            path,
        )?;
        meter.try_reserve_exact(
            &mut generic,
            count as u64,
            std::mem::size_of::<PersistentGenericTypeId>() as u64,
            path,
        )?;
        Ok(Self {
            foundation,
            meter,
            path,
            persistent,
            generic,
        })
    }
    pub fn declared(&mut self, access: &DeclarationAccessSourceV1) -> Result<(), DomainError> {
        self.meter.charge_nodes(1, self.path)?;
        self.meter.charge_edges(1, self.path)?;
        let source = access.definition_origin().origin().source();
        match (access.declared_visibility(), access.lexical_owners().last()) {
            (DeclaredVisibilityV1::Public, _) => self.meter.charge_work(1, self.path)?,
            (DeclaredVisibilityV1::Internal, _) => self.push(Constraint::Cone(source.cone()))?,
            (DeclaredVisibilityV1::Private, Some(owner)) => {
                self.push(Constraint::LexicalOwner(*owner))?
            }
            (DeclaredVisibilityV1::Private, None) => {
                self.push(Constraint::Cone(source.cone()))?;
                let bytes = source.logical_path().as_str().len() as u64;
                self.meter.check_semantic_leaf(bytes, self.path)?;
                self.meter.charge_owned_bytes(bytes, self.path)?;
                self.meter.charge_work(bytes, self.path)?;
                self.push(Constraint::File(source.clone()))?;
            }
            (DeclaredVisibilityV1::Protected, Some(owner)) => self.protected(*owner)?,
            (DeclaredVisibilityV1::Protected, None) => return Err(DomainError::ProtectedOwner),
        }
        Ok(())
    }
    fn protected(&mut self, owner: SourceNominalId) -> Result<(), DomainError> {
        query(self.foundation, self.meter, self.path)?;
        let key = self
            .foundation
            .nominal_key(owner)
            .map_err(DomainError::Foundation)?;
        if key.declaration_kind() != SourceDeclarationKind::Class {
            return Err(DomainError::ProtectedClass(owner));
        }
        match owner {
            SourceNominalId::GenericTemplate(id) => {
                self.meter.charge_work(
                    (self.generic.len() as u64 + 1).saturating_mul(65),
                    self.path,
                )?;
                if !self.generic.contains(&id) {
                    self.generic.push(id);
                }
            }
            SourceNominalId::Concrete(id) => {
                let expected = ExactTypeKey::Nominal(id);
                let bytes = scoop_wire::encoded_length(&expected).map_err(DomainError::Encoding)?;
                self.meter.charge_sha256(bytes, self.path)?;
                let exact =
                    PersistentExactTypeId::from_key(&expected).map_err(DomainError::Identity)?;
                self.meter.charge_work(
                    (u64::from(
                        self.foundation
                            .source()
                            .entries()
                            .exact_keys
                            .values()
                            .len()
                            .max(1)
                            .ilog2(),
                    ) + 1)
                        * 65,
                    self.path,
                )?;
                if self
                    .foundation
                    .exact_type_key(exact)
                    .map_err(DomainError::Foundation)?
                    != &expected
                {
                    return Err(DomainError::ExactClass(exact));
                }
                self.push(Constraint::SubclassesOf(exact))?;
            }
        }
        Ok(())
    }
    fn push(&mut self, constraint: Constraint) -> Result<(), DomainError> {
        let length = scoop_wire::encoded_length(&constraint).map_err(DomainError::Encoding)?;
        let mut work = length.saturating_mul(self.persistent.len() as u64 + 1);
        for previous in &self.persistent {
            work = work.saturating_add(
                scoop_wire::encoded_length(previous).map_err(DomainError::Encoding)?,
            );
        }
        self.meter.charge_work(work, self.path)?;
        if !self.persistent.contains(&constraint) {
            self.persistent.push(constraint);
        }
        Ok(())
    }
    pub fn finish(self) -> Result<DefaultSourceAccessDomainV1, DomainError> {
        for _ in 0..3 {
            self.meter
                .charge_collection_slots(self.persistent.len() as u64, self.path)?;
        }
        for constraint in &self.persistent {
            let length = scoop_wire::encoded_length(constraint).map_err(DomainError::Encoding)?;
            self.meter
                .charge_owned_bytes(length.saturating_mul(2), self.path)?;
            self.meter.charge_work(
                length.saturating_mul(u64::from(self.persistent.len().max(1).ilog2()) + 4),
                self.path,
            )?;
        }
        let count = self.generic.len() as u64;
        self.meter.charge_work(
            count
                .saturating_mul(u64::from(count.max(1).ilog2()) + 1)
                .saturating_mul(65),
            self.path,
        )?;
        let persistent = PersistentAccessDomainV1::try_from_constraints(self.persistent)
            .map_err(DomainError::Domain)?;
        let generic = CanonicalPersistentIdsV1::try_new(self.generic)
            .map_err(DomainError::GenericSubclasses)?;
        DefaultSourceAccessDomainV1::try_new(persistent, generic).map_err(DomainError::Build)
    }
}
