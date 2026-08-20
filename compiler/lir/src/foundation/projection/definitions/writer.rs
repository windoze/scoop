use super::*;
use crate::foundation::definition_subject::member_subject;
use scoop_identity::{
    ObjectDefinitionPlanOwner, OdrMemberDiscriminator as D, OdrMemberRole as R,
    StrongDefinitionRole as S,
};

/// One projection of the actual physical owners, definitions and LIR member delta.
pub(super) struct DefinitionWriter {
    producer: scoop_identity::ConeIdentity,
    owners: BTreeMap<(StrongDefinitionEntity, S), OdrMemberId>,
    pub(super) members: BTreeMap<OdrMemberId, OdrMemberRecord>,
    pub(super) plans: BTreeMap<ObjectDefinitionPlanId, DefinitionPlanRecord>,
    pub(super) atoms: BTreeMap<ObjectDefinitionAtomId, DefinitionAtomRecord>,
    pub(super) symbols: BTreeSet<PersistentSymbolRequest>,
}

impl DefinitionWriter {
    pub(super) fn new(
        module: &Module,
        members: &[OdrMemberRecord],
    ) -> Result<Self, LirFoundationBuildError> {
        let mut writer = Self {
            producer: module.cone,
            owners: BTreeMap::new(),
            members: BTreeMap::new(),
            plans: BTreeMap::new(),
            atoms: BTreeMap::new(),
            symbols: BTreeSet::new(),
        };
        for member in members {
            writer.member(member)?;
            let discriminator = match (member.key().role(), member.key().discriminator()) {
                (R::TypeDescriptor, D::ExactType(id)) => Some(D::ExactType(*id)),
                (R::StaticStorage, D::StaticStorage(id)) => Some(D::StaticStorage(*id)),
                (R::ImmortalObject, D::ImmortalObject(id)) => Some(D::ImmortalObject(*id)),
                _ => None,
            };
            if let Some(discriminator) = discriminator {
                writer.materialization(
                    &crate::MaterializationRoot::prior_stage_odr(member.key().group()),
                    R::RegistrationRecord,
                    discriminator,
                )?;
            }
        }
        for function in module.callable_bodies() {
            let body = &function.callable_body;
            let root = body.materialization_root();
            if let Some(owner) = body.release_owner() {
                writer.materialization(&root, R::ReleaseHook, D::ExactType(owner))?;
            }
            if let ObjectDefinitionPlanOwner::Odr { member } =
                body.definition_plan_key(module.cone).owner()
            {
                writer.owner(
                    StrongDefinitionEntity::callable_body(body.id()),
                    S::CallableBody,
                    member,
                )?;
            }
            writer.materialization(&root, R::RegistrationRecord, D::CallableBody(body.id()))?;
            for safepoint in function.safepoints.iter() {
                writer.materialization(
                    &root,
                    R::RegistrationRecord,
                    D::SafepointSite(safepoint.site_id()),
                )?;
            }
        }
        for (_, unit) in module.initialization_units.iter() {
            if let Some(key) = unit.identity.key().specialization_key() {
                let group =
                    OdrGroupId::from_key(&key).map_err(LirFoundationBuildError::DefinitionHash)?;
                let root = crate::MaterializationRoot::prior_stage_odr(group);
                for role in [R::InitializationCell, R::RegistrationRecord] {
                    writer.materialization(
                        &root,
                        role,
                        D::InitializationUnit(unit.identity.id()),
                    )?;
                }
            }
        }
        Ok(writer)
    }

    fn owner(
        &mut self,
        entity: StrongDefinitionEntity,
        role: S,
        member: OdrMemberId,
    ) -> Result<(), LirFoundationBuildError> {
        if let Some(first) = self.owners.insert((entity, role), member)
            && first != member
        {
            return Err(LirFoundationBuildError::ConflictingDefinitionOwner {
                entity,
                role,
                first,
                second: member,
            });
        }
        Ok(())
    }

    fn member(&mut self, member: &OdrMemberRecord) -> Result<(), LirFoundationBuildError> {
        if let Some((entity, role)) = member_subject(member.key()) {
            self.owner(entity, role, member.id())?;
        }
        insert_projected_identity(
            &mut self.members,
            Some(member),
            LirFoundationTable::OdrMember,
        )
    }

    fn materialization(
        &mut self,
        root: &crate::MaterializationRoot,
        role: R,
        discriminator: D,
    ) -> Result<(), LirFoundationBuildError> {
        let identity = root
            .materialization(role, discriminator)
            .map_err(LirFoundationBuildError::DefinitionHash)?;
        if let Some(member) = identity.odr_member_record() {
            self.member(member)?;
        }
        Ok(())
    }

    pub(super) fn define(
        &mut self,
        entity: StrongDefinitionEntity,
        role: S,
        associated: Vec<(DefinitionAtomRole, DefinitionAtomSubkey)>,
    ) -> Result<(), LirFoundationBuildError> {
        let (key, linkage) = match self.owners.get(&(entity, role)) {
            Some(&member) => (ObjectDefinitionPlanKey::odr(member), LinkageClass::OdrWeak),
            None => (
                ObjectDefinitionPlanKey::strong(self.producer, entity, role)
                    .map_err(LirFoundationBuildError::DefinitionIdentity)?,
                LinkageClass::ConeStrong,
            ),
        };
        let symbol =
            entity
                .primary_symbol_key(role)
                .ok_or(LirFoundationBuildError::DefinitionIdentity(
                    ObjectDefinitionIdentityError::StrongRoleEntityMismatch,
                ))?;
        self.symbols.insert(
            PersistentSymbolRequest::new(symbol, linkage)
                .map_err(LirFoundationBuildError::SymbolRequest)?,
        );
        let plan =
            CborIdentityRecord::from_key(key).map_err(LirFoundationBuildError::DefinitionHash)?;
        let plan_id = plan.id();
        insert_projected_identity(
            &mut self.plans,
            Some(&plan),
            LirFoundationTable::DefinitionPlan,
        )?;
        let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            plan_id,
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .map_err(LirFoundationBuildError::DefinitionHash)?;
        insert_projected_identity(
            &mut self.atoms,
            Some(&primary),
            LirFoundationTable::DefinitionAtom,
        )?;
        for (role, subkey) in associated {
            let atom =
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(plan_id, role, subkey))
                    .map_err(LirFoundationBuildError::DefinitionHash)?;
            insert_projected_identity(
                &mut self.atoms,
                Some(&atom),
                LirFoundationTable::DefinitionAtom,
            )?;
        }
        Ok(())
    }
}
