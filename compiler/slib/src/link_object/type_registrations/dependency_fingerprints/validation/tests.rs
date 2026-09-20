use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, ExactTypeKey, MangledSymbol};

fn exact(nominal: CoreBuiltinNominal) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal.identity_record().id())).unwrap()
}

#[test]
fn a_parent_relocation_must_match_the_exact_external_descriptor_symbol() {
    let unit = PersistentSymbolKey::TypeDescriptor(exact(CoreBuiltinNominal::Unit));
    let other = PersistentSymbolKey::TypeDescriptor(exact(CoreBuiltinNominal::Any));
    let target = VerifiedRelocationTargetV1::ExternalUndefined {
        table_index: 7,
        name: format!("_{}", MangledSymbol::from_key(&unit)).into_bytes(),
    };
    assert!(external_target_matches(&target, unit));
    assert!(!external_target_matches(&target, other));
}

#[test]
fn a_local_relocation_cannot_substitute_another_definition_owner_or_role() {
    let producer = ConeIdentity::SINGLE_FILE;
    let entity = StrongDefinitionEntity::exact_type(exact(CoreBuiltinNominal::Unit));
    let role = StrongDefinitionRole::TypeDescriptor;
    let target = VerifiedRelocationTargetV1::StrongDefinition {
        definition: ObjectDefinitionPlanId::from_key(
            &ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
        )
        .unwrap(),
    };
    assert!(strong_target_matches(&target, producer, entity, role));
    assert!(!strong_target_matches(
        &target,
        ConeIdentity::CORE,
        entity,
        role
    ));
    assert!(!strong_target_matches(
        &target,
        producer,
        entity,
        StrongDefinitionRole::TypeRegistration
    ));
    assert!(!strong_target_matches(
        &target,
        producer,
        StrongDefinitionEntity::exact_type(exact(CoreBuiltinNominal::Any)),
        role,
    ));
}
