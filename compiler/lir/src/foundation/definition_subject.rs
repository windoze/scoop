//! Physical definition lookup through existing resolved identity keys.

use super::{ConeLirFoundation, DefinitionPlanRecord};
use scoop_identity::{
    CallableBodyKeyKind, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, OdrMemberDiscriminator as D, OdrMemberKey,
    OdrMemberRole as R, StrongDefinitionEntity, StrongDefinitionRole as S,
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
        let key = ObjectDefinitionPlanKey::strong(self.producer(), entity, role).ok()?;
        let id = ObjectDefinitionPlanId::from_key(&key).ok()?;
        if let Some(record) = self.definition_plan(id) {
            return Some(record);
        }
        let member = if role == S::CallableBody {
            let scoop_identity::StrongDefinitionEntityKind::CallableBody(body) = entity.kind()
            else {
                return None;
            };
            // Callable bodies follow dependency order.
            let body = self
                .callable_bodies()
                .iter()
                .find(|record| record.id() == body)?;
            match body.key().kind() {
                CallableBodyKeyKind::Odr(member) => member.member(),
                CallableBodyKeyKind::ReleaseHook { .. } => self
                    .as_canonical()
                    .odr_members
                    .iter()
                    .find(|record| member_subject(record.key()) == Some((entity, role)))?
                    .id(),
                _ => return None,
            }
        } else {
            self.as_canonical()
                .odr_members
                .iter()
                .find(|record| member_subject(record.key()) == Some((entity, role)))?
                .id()
        };
        let id = ObjectDefinitionPlanId::from_key(&ObjectDefinitionPlanKey::odr(member)).ok()?;
        self.definition_plan(id)
    }
}

pub(super) fn member_subject(key: &OdrMemberKey) -> Option<(StrongDefinitionEntity, S)> {
    let (entity, role) = match (key.role(), key.discriminator()) {
        (R::ReleaseHook, D::ExactType(owner)) => (
            StrongDefinitionEntity::callable_body(
                scoop_identity::PersistentCallableBodyId::from_key(
                    &scoop_identity::CallableBodyKey::release_hook(*owner),
                )
                .expect("an exact owner derives a fixed-size release body key"),
            ),
            S::CallableBody,
        ),
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
