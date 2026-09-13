use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::{ConeImagePlanBuildError, ConeImagePlanV1, DecodedConeImagePlanV1};
use crate::{
    CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, OdrFreeLirFoundation,
    StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};

#[test]
fn single_file_image_plan_binds_coordinate_core_dependency_and_empty_tables() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate.clone(),
        &foundation,
        &registrations,
        &digest_plan,
    )
    .unwrap();

    assert_eq!(plan.cone().coordinate(), &coordinate);
    assert_eq!(plan.cone().identity(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plan.dependencies(), &[ConeIdentity::CORE]);
    assert!(plan.tables().static_storages().is_empty());
    assert!(plan.tables().immortal_objects().is_empty());
    assert!(plan.tables().initialization_units().is_empty());
    assert!(plan.tables().type_registrations().is_empty());
    assert!(plan.tables().safepoints().is_empty());
    assert!(plan.tables().callables().is_empty());
    assert_eq!(plan.symbol().linkage(), LinkageClass::ConeStrong);

    let bytes = encode(&plan).unwrap();
    let decoded: DecodedConeImagePlanV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let validated = decoded
        .validate(&coordinate, &foundation, &registrations, &digest_plan)
        .unwrap();
    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(
        hex(&bytes),
        "a601a201a3016573636f6f70026b73696e676c652d66696c650365302e302e3002582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b6028158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d03a601800280038004800580068004a201a2001101582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b602010558200a6b2c5d4d7a6df125364db1b8299514682385fbb59b91bc4cc661d456f8d12c06582072888c00464fa258dfb42b658f906c21cc9ded31ce0a4077616ee2f0caa402c1"
    );
}

#[test]
fn core_image_has_no_dependency_edge() {
    let coordinate = ConeCoordinate::reserved_core();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let plan = ConeImagePlanV1::new(coordinate, &foundation, &registrations, &digest_plan).unwrap();

    assert!(plan.dependencies().is_empty());
}

#[test]
fn image_requires_every_registration_fingerprint_as_a_direct_input() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let exact = unit_exact_type();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), Some(exact), false);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();

    assert!(matches!(
        ConeImagePlanV1::new(coordinate, &foundation, &registrations, &digest_plan),
        Err(ConeImagePlanBuildError::RegistrationInputs { .. })
    ));
}

#[test]
fn reader_rejects_a_coordinate_from_another_cone() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate.clone(),
        &foundation,
        &registrations,
        &digest_plan,
    )
    .unwrap();
    let decoded: DecodedConeImagePlanV1 =
        decode_canonical(&encode(&plan).unwrap(), DecodeLimits::default()).unwrap();

    assert!(
        decoded
            .validate(
                &ConeCoordinate::reserved_core(),
                &foundation,
                &registrations,
                &digest_plan,
            )
            .is_err()
    );
}

fn image_fixture(
    coordinate: ConeCoordinate,
    registration: Option<PersistentExactTypeId>,
    image_depends_on_registration: bool,
) -> (OdrFreeLirFoundation, StrongDigestFinalizationPlanV1) {
    let producer = coordinate.identity().unwrap();
    let image_plan_key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::cone_image(producer),
        StrongDefinitionRole::ImageDescriptor,
    )
    .unwrap();
    let image_plan = CborIdentityRecord::from_key(image_plan_key).unwrap();
    let image_plan_id = image_plan.id();
    let image_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        image_plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let image_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();

    let mut plans = vec![image_plan];
    let mut nodes = Vec::new();
    if let Some(exact) = registration {
        let registration_plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
            )
            .unwrap(),
        )
        .unwrap();
        let registration_node = DigestNodeV1::new(
            DigestNodeKey::strong_registration(registration_plan.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        plans.push(registration_plan);
        nodes.push(registration_node);
    }

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(vec![image_atom]).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![image_symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);
    let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
    let image_patch = DigestPatchIntentKey::new(
        image_node_id,
        image_plan_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let inputs = if image_depends_on_registration {
        nodes.iter().map(DigestInputRefV1::from_node).collect()
    } else {
        Vec::new()
    };
    let image_node = DigestNodeV1::new(image_key, inputs, vec![image_patch]).unwrap();
    nodes.push(image_node);
    let digest_plan = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    (foundation, digest_plan)
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
