use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, ExactTypeKey, ObjectDefinitionPlanKey,
    PersistentExactTypeId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::{
    DecodedRegistrationIdentitySurfaceV1, RegistrationIdentitySurfaceV1,
    RegistrationIdentityValidationError, RegistrationTableV1,
};
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn six_empty_tables_have_a_fixed_wire_shape() {
    let foundation = empty_foundation();
    let surface = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert!(surface.static_storages().is_empty());
    assert!(surface.immortal_objects().is_empty());
    assert!(surface.initialization_units().is_empty());
    assert!(surface.type_registrations().is_empty());
    assert!(surface.safepoints().is_empty());
    assert!(surface.callables().is_empty());
    assert_eq!(
        hex(&encode(&surface).unwrap()),
        "a6018002800380048005800680"
    );
}

#[test]
fn registration_role_derives_typed_table_and_plan() {
    let exact = unit_exact_type();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let surface = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert_eq!(surface.type_registrations().len(), 1);
    let entry = surface.type_registrations()[0];
    assert_eq!(entry.semantic_id(), exact);
    assert_eq!(entry.definition_plan(), plan.id());

    let bytes = encode(&surface).unwrap();
    let decoded: DecodedRegistrationIdentitySurfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        encode(&decoded.validate(&foundation).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn reader_rebuilds_tables_instead_of_accepting_missing_entries() {
    let exact = unit_exact_type();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let surface = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let mut decoded: DecodedRegistrationIdentitySurfaceV1 =
        decode_canonical(&encode(&surface).unwrap()).unwrap();
    decoded.type_registrations.clear();

    assert_eq!(
        decoded.validate(&foundation),
        Err(RegistrationIdentityValidationError::TableLength {
            table: RegistrationTableV1::Type,
            expected: 1,
            actual: 0,
        })
    );
}

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
