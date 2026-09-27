use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DigestNodeKey, ExactTypeKey,
    ObjectDefinitionPlanKey, PersistentExactTypeId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::{
    DecodedStrongRegistrationIdentitySurfaceV1, RegistrationTableV1,
    StrongRegistrationIdentityBuildError, StrongRegistrationIdentitySurfaceV1,
    StrongRegistrationIdentityValidationError,
};
use crate::{
    CanonicalLirFoundation, ConeLirFoundation, DigestInputRefV1, DigestNodeV1,
    StrongDigestFinalizationPlanV1,
};

#[test]
fn six_empty_tables_have_a_fixed_wire_shape() {
    let foundation = empty_foundation();
    let digest_plan = digest_plan(&foundation, Vec::new());
    let surface =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();

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
fn registration_role_derives_typed_table_plan_and_digest_node() {
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
    let registration = DigestNodeV1::new(
        DigestNodeKey::strong_registration(plan.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let expected_node = registration.id();
    let digest_plan = digest_plan(&foundation, vec![registration]);
    let surface =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();

    assert_eq!(surface.type_registrations().len(), 1);
    let entry = surface.type_registrations()[0];
    assert_eq!(entry.semantic_id(), exact);
    assert_eq!(entry.definition_plan(), plan.id());
    assert_eq!(entry.fingerprint_node(), expected_node);

    let bytes = encode(&surface).unwrap();
    let decoded: DecodedStrongRegistrationIdentitySurfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        encode(&decoded.validate(&foundation, &digest_plan).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn every_registration_plan_requires_its_own_strong_digest_node() {
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
    let digest_plan = digest_plan(&foundation, Vec::new());

    assert_eq!(
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan),
        Err(StrongRegistrationIdentityBuildError::MissingFingerprintNode(plan.id()))
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
    let registration = DigestNodeV1::new(
        DigestNodeKey::strong_registration(plan.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let digest_plan = digest_plan(&foundation, vec![registration]);
    let surface =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let mut decoded: DecodedStrongRegistrationIdentitySurfaceV1 =
        decode_canonical(&encode(&surface).unwrap()).unwrap();
    decoded.type_registrations.clear();

    assert_eq!(
        decoded.validate(&foundation, &digest_plan),
        Err(StrongRegistrationIdentityValidationError::TableLength {
            table: RegistrationTableV1::Type,
            expected: 1,
            actual: 0,
        })
    );
}

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

fn digest_plan(
    foundation: &ConeLirFoundation,
    registrations: Vec<DigestNodeV1>,
) -> StrongDigestFinalizationPlanV1 {
    let inputs = registrations
        .iter()
        .map(DigestInputRefV1::from_node)
        .collect();
    let image = DigestNodeV1::new(
        DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
        inputs,
        Vec::new(),
    )
    .unwrap();
    let mut nodes = registrations;
    nodes.push(image);
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
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
