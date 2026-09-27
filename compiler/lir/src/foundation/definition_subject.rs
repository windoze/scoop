//! Physical definition lookup through existing resolved identity keys.

use super::{ConeLirFoundation, DefinitionPlanRecord};
use scoop_identity::{
    CallableBodyKeyKind, DigestNodeKey, ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole,
    OdrMemberDiscriminator as D, OdrMemberKey, OdrMemberRole as R, StrongDefinitionEntity,
    StrongDefinitionRole as S,
};

impl ConeLirFoundation {
    pub(crate) fn definition_subject(
        &self,
        record: &DefinitionPlanRecord,
    ) -> Option<(StrongDefinitionEntity, S)> {
        match (record.key().owner(), record.key().definition_role()) {
            (ObjectDefinitionPlanOwner::Strong { entity, .. }, ObjectDefinitionPlanRole::Strong(role)) => Some((entity, role)),
            (ObjectDefinitionPlanOwner::Odr { member }, ObjectDefinitionPlanRole::OdrMemberPrimary) => {
                self.callable_bodies().iter().find_map(|body| {
                    matches!(body.key().kind(), CallableBodyKeyKind::Odr(id) if id.member() == member)
                        .then_some((StrongDefinitionEntity::callable_body(body.id()), S::CallableBody))
                }).or_else(|| self.as_canonical().odr_members.iter()
                    .find(|record| record.id() == member)
                    .and_then(|record| member_subject(record.key())))
            }
            _ => None,
        }
    }

    pub(crate) fn definition_for(
        &self,
        entity: StrongDefinitionEntity,
        role: S,
    ) -> Option<&DefinitionPlanRecord> {
        if let Some(record) = self.definition_plans().iter().find(|record| {
            matches!(record.key().owner(), ObjectDefinitionPlanOwner::Strong { entity: candidate, .. } if candidate == entity)
                && record.key().definition_role() == ObjectDefinitionPlanRole::Strong(role)
        }) {
            return Some(record);
        }
        let member = if role == S::CallableBody {
            let scoop_identity::StrongDefinitionEntityKind::CallableBody(body) = entity.kind()
            else {
                return None;
            };
            let body = self
                .callable_bodies()
                .iter()
                .find(|record| record.id() == body)?;
            let CallableBodyKeyKind::Odr(member) = body.key().kind() else {
                return None;
            };
            member.member()
        } else {
            self.as_canonical()
                .odr_members
                .iter()
                .find(|record| member_subject(record.key()) == Some((entity, role)))?
                .id()
        };
        self.definition_plans()
            .iter()
            .find(|record| record.key().owner() == ObjectDefinitionPlanOwner::Odr { member })
    }

    pub(crate) fn registration_digest_key(&self, record: &DefinitionPlanRecord) -> DigestNodeKey {
        match record.key().owner() {
            ObjectDefinitionPlanOwner::Strong { .. } => {
                DigestNodeKey::strong_registration(record.id())
            }
            ObjectDefinitionPlanOwner::Odr { member } => {
                DigestNodeKey::odr_member_definition(member)
            }
        }
    }
}

pub(super) fn member_subject(key: &OdrMemberKey) -> Option<(StrongDefinitionEntity, S)> {
    let (entity, role) = match (key.role(), key.discriminator()) {
        (R::Layout, D::Layout(id)) => (StrongDefinitionEntity::layout(*id), S::Layout),
        (R::ScanProgram, D::Scan(id)) => (StrongDefinitionEntity::scan(*id), S::ScanProgram),
        (R::TypeDescriptor, D::ExactType(id)) => {
            (StrongDefinitionEntity::exact_type(*id), S::TypeDescriptor)
        }
        (R::DispatchTable, D::DispatchTable(id)) => (
            StrongDefinitionEntity::dispatch_table(*id),
            S::DispatchTable,
        ),
        (R::StaticStorage, D::StaticStorage(id)) => (
            StrongDefinitionEntity::static_storage(*id),
            S::StaticStorage,
        ),
        (R::ImmortalObject, D::ImmortalObject(id)) => (
            StrongDefinitionEntity::immortal_object(*id),
            S::ImmortalObject,
        ),
        (R::InitializationCell, D::InitializationUnit(id)) => (
            StrongDefinitionEntity::initialization_unit(*id),
            S::InitializationCell,
        ),
        (R::InitializationDescriptor, D::InitializationUnit(id)) => (
            StrongDefinitionEntity::initialization_unit(*id),
            S::InitializationDescriptor,
        ),
        (R::RegistrationRecord, D::CallableBody(id)) => (
            StrongDefinitionEntity::callable_body(*id),
            S::CallableRegistration,
        ),
        (R::RegistrationRecord, D::SafepointSite(id)) => (
            StrongDefinitionEntity::safepoint_site(*id),
            S::SafepointRegistration,
        ),
        (R::RegistrationRecord, D::ExactType(id)) => {
            (StrongDefinitionEntity::exact_type(*id), S::TypeRegistration)
        }
        (R::RegistrationRecord, D::StaticStorage(id)) => (
            StrongDefinitionEntity::static_storage(*id),
            S::RootRegistration,
        ),
        (R::RegistrationRecord, D::ImmortalObject(id)) => (
            StrongDefinitionEntity::immortal_object(*id),
            S::ImmortalRegistration,
        ),
        (R::RegistrationRecord, D::InitializationUnit(id)) => (
            StrongDefinitionEntity::initialization_unit(*id),
            S::InitializationRegistration,
        ),
        _ => return None,
    };
    Some((entity, role))
}
