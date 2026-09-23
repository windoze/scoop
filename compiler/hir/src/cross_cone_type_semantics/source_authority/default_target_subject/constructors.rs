use super::*;

impl DefaultTargetIdentityQueriesV1<'_> {
    /// Resolves the access declaration and checks the applied nominal root.
    /// Full argument types, parameter protocols and adapter bodies require
    /// their own contracts before any constructor can execute.
    pub fn default_constructor_access_subject(
        &self,
        target: &DefaultConstructorRefV1,
        meter: &mut BudgetMeter,
    ) -> Result<Subject, Error> {
        let mut query = Query {
            foundation: self,
            meter,
            path: WirePath::root(),
        };
        query.meter.check_semantic_depth(1, &query.path)?;
        query.meter.charge_nodes(1, &query.path)?;
        let (subject, owner) = match target {
            DefaultConstructorRefV1::Struct { declaration, .. } => {
                query.constructor(*declaration, SourceDeclarationKind::Struct)?
            }
            DefaultConstructorRefV1::Class { declaration, .. } => {
                let declaration = match declaration {
                    DefaultClassConstructorIdV1::Source(id) => *id,
                    DefaultClassConstructorIdV1::Generated(id) => query.adapter(*id)?,
                };
                query.constructor(declaration, SourceDeclarationKind::Class)?
            }
            DefaultConstructorRefV1::Variant { declaration, .. } => {
                let canonical = self.foundation.as_canonical();
                let target = Target::EnumVariant(*declaration);
                let key = query.key(
                    canonical.type_source_enum_variant_records(),
                    *declaration,
                    || Error::MissingTarget(target),
                )?;
                let owner = key.source_owner().ok_or(Error::Role(target))?;
                (query.nominal(owner, SourceDeclarationKind::Enum)?, owner)
            }
        };
        query.applied_owner(owner, target.owner_type())?;
        Ok(subject)
    }
}
impl Query<'_, '_, '_> {
    fn adapter(
        &mut self,
        id: PersistentGeneratedCallableId,
    ) -> Result<PersistentConstructorId, Error> {
        self.meter.charge_edges(1, &self.path)?;
        let canonical = self.foundation.foundation.as_canonical();
        let key = self.key(
            canonical.type_source_generated_callable_records(),
            id,
            || Error::MissingAdapter(id),
        )?;
        match key {
            GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                Ok(*constructor)
            }
            _ => Err(Error::AdapterRole(id)),
        }
    }
    fn constructor(
        &mut self,
        id: PersistentConstructorId,
        kind: SourceDeclarationKind,
    ) -> Result<(Subject, SourceNominalId), Error> {
        self.meter.charge_edges(1, &self.path)?;
        let subject = Subject::Constructor(id);
        let key = self.declaration(subject)?;
        let owner = match key.owners().owners().last() {
            Some(DefinitionOwnerAtom::Type(id)) => SourceNominalId::Concrete(*id),
            Some(DefinitionOwnerAtom::GenericType(id)) => SourceNominalId::GenericTemplate(*id),
            _ => return Err(Error::ConstructorOwner(id)),
        };
        self.nominal(owner, kind)?;
        Ok((subject, owner))
    }
}
