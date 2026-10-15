use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole, CoreBuiltinNominal,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::encode_runtime;

use super::{ConeImagePlanBuildError, ConeImagePlanV1, ConeRecordV1};
use crate::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1,
    DigestNodeV1, RegistrationIdentitySurfaceV1,
};

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
