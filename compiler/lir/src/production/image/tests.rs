use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole, CoreBuiltinNominal,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode, encode_runtime};

use super::{ConeImagePlanBuildError, ConeImagePlanV1, ConeRecordV1, DecodedConeImagePlanV1};
use crate::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1,
    DigestNodeV1, RegistrationIdentitySurfaceV1,
};

#[test]
fn single_file_image_plan_binds_coordinate_core_dependency_and_empty_tables() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true, true);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate.clone(),
        &[scoop_identity::ConeIdentity::CORE],
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
    assert_ne!(
        plan.support_atoms().dependencies(),
        plan.support_atoms().static_storages()
    );

    let bytes = encode(&plan).unwrap();
    let decoded: DecodedConeImagePlanV1 = decode_canonical(&bytes).unwrap();
    let validated = decoded
        .validate(
            &coordinate,
            &[ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digest_plan,
        )
        .unwrap();
    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(
        hex(&bytes),
        "a701a201a3016573636f6f70026b73696e676c652d66696c650365302e302e3002582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b6028158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d03a601800280038004800580068004a201a2001101582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b602010558200a6b2c5d4d7a6df125364db1b8299514682385fbb59b91bc4cc661d456f8d12c06ac01582073f2fdcb7429fe8bd11f3fc99126ad7a633b3dd7923907e9725a3a0d105508a30258206201bf4b3657fdcdc2027baedd4e7115fe96c99cd03543e2ca050fe4fdfd31d1035820b34ed9537b06e55fe3759051e00d7a5fcb4463826571c120b5373ad0b5785b5004582023697c2b6819a50f041a0ccef1821dbd911e8d97d0c17e19422d9748af13f682055820b77bcdfe99e5148caf8a888e888410ed9be43294cf02378aa1097a8ccfb136c2065820fa6b3ba4eb80b07fc0ba7b7f170dcc8ef39a995ca2dca206dca4cf58451251c10758201cac9a0d4cb821b0b8b71aab4066e8ab44b278a29ec2a511a754bad6f7432aaf085820e486529b0103087625514cf0a890a15fc954aece66801177ed942cac2f23996d0958201a3312e5ae116d0648c83edc2989ca558541d7c3eb6f1d8cc8d6bf288fed06fd0a582088a456ea6eeae7001794f6cd99f19a749e3d751842f46074d7334f27cf831ece0b58203d175a2054423000b95a7c61c1f7285e43d3bf0aca26402bf0c04ff130c0e7420c58206544eb65909af6ed630ad399d909303de3bebc2c7fcb7951e57c459224dd233307582072888c00464fa258dfb42b658f906c21cc9ded31ce0a4077616ee2f0caa402c1"
    );
}

#[test]
fn core_image_preserves_an_explicit_empty_dependency_set() {
    let coordinate = ConeCoordinate::reserved_core();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true, true);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let plan =
        ConeImagePlanV1::new(coordinate, &[], &foundation, &registrations, &digest_plan).unwrap();

    assert!(plan.dependencies().is_empty());
}

#[test]
fn cone_record_runtime_encoding_uses_declared_field_order() {
    let record = ConeRecordV1::new(ConeCoordinate::reserved_single_file()).unwrap();
    let mut expected = Vec::new();
    for value in [b"scoop".as_slice(), b"single-file", b"0.0.0"] {
        expected.extend_from_slice(&(value.len() as u64).to_le_bytes());
        expected.extend_from_slice(value);
    }
    expected.extend_from_slice(ConeIdentity::SINGLE_FILE.as_array());

    assert_eq!(encode_runtime(&record).unwrap(), expected);
}

#[test]
fn image_requires_every_record_digest_field_as_a_direct_input() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let exact = unit_exact_type();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), Some(exact), false, true);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert!(matches!(
        ConeImagePlanV1::new(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digest_plan
        ),
        Err(ConeImagePlanBuildError::RegistrationInputs { .. })
    ));
}

#[test]
fn reader_rejects_a_coordinate_from_another_cone() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true, true);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate.clone(),
        &[scoop_identity::ConeIdentity::CORE],
        &foundation,
        &registrations,
        &digest_plan,
    )
    .unwrap();
    let decoded: DecodedConeImagePlanV1 = decode_canonical(&encode(&plan).unwrap()).unwrap();

    assert!(
        decoded
            .validate(
                &ConeCoordinate::reserved_core(),
                &[],
                &foundation,
                &registrations,
                &digest_plan,
            )
            .is_err()
    );
}

#[test]
fn image_rejects_the_obsolete_primary_only_atom_shape() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true, false);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert!(matches!(
        ConeImagePlanV1::new(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digest_plan
        ),
        Err(ConeImagePlanBuildError::AtomSet { .. })
    ));
}

#[test]
fn image_rejects_non_registration_direct_inputs() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digest_plan) = image_fixture(coordinate.clone(), None, true, true);
    let primary = foundation
        .resolve_definition_atom(
            foundation.definition_plans()[0].id(),
            DefinitionAtomRole::Primary,
        )
        .unwrap()
        .1;
    let extra = DigestNodeV1::new(
        DigestNodeKey::object_definition(primary),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let image = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::runtime_image(coordinate.identity().unwrap()))
        .unwrap();
    let image = DigestNodeV1::new(
        *image.key(),
        vec![DigestInputRefV1::from_node(&extra)],
        image
            .patch_intents()
            .iter()
            .map(|patch| *patch.key())
            .collect(),
    )
    .unwrap();
    let digest_plan = DigestFinalizationPlanV1::new(vec![extra, image], &foundation).unwrap();
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert!(matches!(
        ConeImagePlanV1::new(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digest_plan
        ),
        Err(ConeImagePlanBuildError::RegistrationInputs { .. })
    ));
}

pub(super) fn image_fixture(
    coordinate: ConeCoordinate,
    registration: Option<PersistentExactTypeId>,
    image_depends_on_registration: bool,
    include_support_atoms: bool,
) -> (ConeLirFoundation, DigestFinalizationPlanV1) {
    let producer = coordinate.identity().unwrap();
    let image_plan_key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::cone_image(producer),
        StrongDefinitionRole::ImageDescriptor,
    )
    .unwrap();
    let image_plan = CborIdentityRecord::from_key(image_plan_key).unwrap();
    let image_plan_id = image_plan.id();
    let mut image_atoms = image_atoms(image_plan.id());
    if !include_support_atoms {
        image_atoms.truncate(1);
    }
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
        let descriptor = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
            )
            .unwrap(),
        )
        .unwrap();
        let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
        let registration_primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            registration_plan.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
        let key = DigestNodeKey::object_definition(primary.id());
        let node = DigestNodeId::from_key(&key).unwrap();
        nodes.push(
            DigestNodeV1::new(
                key,
                Vec::new(),
                vec![DigestPatchIntentKey::new(
                    node,
                    registration_plan.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::DescriptorDefinition,
                )],
            )
            .unwrap(),
        );
        image_atoms.extend([primary, registration_primary]);
        plans.push(descriptor);
        plans.push(registration_plan);
    }

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(image_atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![image_symbol]).unwrap());
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();

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
    let digest_plan = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    (foundation, digest_plan)
}

fn image_atoms(
    plan: scoop_identity::ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let mut keys = vec![ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    )];
    keys.extend(
        [
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateGroup,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateName,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateVersion,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Dependencies,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::StaticStorages,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::ImmortalObjects,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::InitializationUnits,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::TypeRegistrations,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Safepoints,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Callables,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArrayBoundsMessage,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArraySizeOverflowMessage,
            ),
        ]
        .map(|(role, support)| {
            ObjectDefinitionAtomKey::new(
                plan,
                role,
                DefinitionAtomSubkey::ConeImageSupport(support),
            )
        }),
    );
    keys.into_iter()
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
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
