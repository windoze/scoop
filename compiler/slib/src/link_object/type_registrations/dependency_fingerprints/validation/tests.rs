use super::*;
use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, ExactTypeKey, MangledSymbol, ObjectDefinitionPlanKey,
};

fn exact(nominal: CoreBuiltinNominal) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal.identity_record().id())).unwrap()
}

#[test]
fn a_parent_relocation_must_match_the_exact_external_descriptor_symbol() {
    let unit = PersistentSymbolKey::TypeDescriptor(exact(CoreBuiltinNominal::Unit));
    let other = PersistentSymbolKey::TypeDescriptor(exact(CoreBuiltinNominal::Any));
    for profile in [
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        scoop_lir::LirTargetProfile::from_id(scoop_identity::TargetProfileId::LinuxX86_64Gnu),
        scoop_lir::LirTargetProfile::from_id(scoop_identity::TargetProfileId::LinuxX86_64Musl),
    ] {
        let target = VerifiedRelocationTargetV1::ExternalUndefined {
            table_index: 7,
            name: profile
                .contract()
                .native_symbol_normalization()
                .compiler_generated_object_symbol(MangledSymbol::from_key(&unit).as_str())
                .into_bytes(),
        };
        assert!(external_target_matches(profile, &target, unit));
        assert!(!external_target_matches(profile, &target, other));
    }
}

#[test]
fn a_separate_hook_object_is_referenced_by_its_exact_callable_body_symbol() {
    let hook = |owner| {
        scoop_identity::PersistentCallableBodyId::from_key(
            &scoop_identity::CallableBodyKey::release_hook(exact(owner)),
        )
        .unwrap()
    };
    let expected = PersistentSymbolKey::CallableBody(hook(CoreBuiltinNominal::Unit));
    let other = PersistentSymbolKey::CallableBody(hook(CoreBuiltinNominal::Any));
    for profile in [
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        scoop_lir::LirTargetProfile::from_id(scoop_identity::TargetProfileId::LinuxX86_64Gnu),
        scoop_lir::LirTargetProfile::from_id(scoop_identity::TargetProfileId::LinuxX86_64Musl),
    ] {
        let target = VerifiedRelocationTargetV1::ExternalUndefined {
            table_index: 4,
            name: profile
                .contract()
                .native_symbol_normalization()
                .compiler_generated_object_symbol(MangledSymbol::from_key(&expected).as_str())
                .into_bytes(),
        };
        assert!(external_target_matches(profile, &target, expected));
        assert!(!external_target_matches(profile, &target, other));
    }
}

#[test]
fn a_local_relocation_cannot_substitute_another_definition_owner_or_role() {
    let producer = ConeIdentity::SINGLE_FILE;
    let entity = StrongDefinitionEntity::exact_type(exact(CoreBuiltinNominal::Unit));
    let role = StrongDefinitionRole::TypeDescriptor;
    let definition = ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
    )
    .unwrap();
    let target = VerifiedRelocationTargetV1::StrongDefinition { definition };
    let definitions = [(definition, (entity, role))].into_iter().collect();
    assert!(definition_target_matches(
        &target,
        &definitions,
        entity,
        role
    ));
    let other = VerifiedRelocationTargetV1::StrongDefinition {
        definition: ObjectDefinitionPlanId::from_key(
            &ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).unwrap(),
        )
        .unwrap(),
    };
    assert!(!definition_target_matches(
        &other,
        &definitions,
        entity,
        role
    ));
    assert!(!definition_target_matches(
        &target,
        &definitions,
        entity,
        StrongDefinitionRole::TypeRegistration
    ));
    assert!(!definition_target_matches(
        &target,
        &definitions,
        StrongDefinitionEntity::exact_type(exact(CoreBuiltinNominal::Any)),
        role,
    ));
}
