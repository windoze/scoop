use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DefinitionAtomRole, DefinitionAtomSubkey,
    DigestKind, DigestNodeId, DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole,
    ExactTypeKey, LayoutKey, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PendingIdentityValidation, PersistentExactTypeId, RepresentationRole, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::{
    DecodedStrongDigestFinalizationPlanV1, DigestInputRefV1, DigestNodeBuildError, DigestNodeV1,
    DigestPlanError, StrongDigestFinalizationPlanV1, StrongDigestPlanBuildError,
    StrongDigestPlanValidationError,
};
use crate::{CanonicalLirFoundation, OdrFreeLirFoundation};

#[test]
fn runtime_image_only_plan_has_canonical_wire_and_round_trips() {
    let foundation = empty_foundation();
    let image = image_node(Vec::new());
    let plan = StrongDigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();
    let bytes = encode(&plan).unwrap();
    let decoded: DecodedStrongDigestFinalizationPlanV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let validated = decoded.validate(&mut identities, &foundation).unwrap();

    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(validated.nodes().len(), 1);
    assert_eq!(validated.nodes()[0].kind(), DigestKind::RuntimeImage);
    assert_eq!(
        hex(&bytes),
        "81a301a201582064065d7cecf340fb84eaf79a1de2a018d762b8597b0f5a95a5b9c8e78fae03ca02a2010a02a2000a01582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b602800380"
    );
}

#[test]
fn trusted_builder_sorts_nodes_and_accepts_only_typed_legal_edges() {
    let (foundation, layout) = layout_foundation();
    let layout_node =
        DigestNodeV1::new(DigestNodeKey::layout(layout), Vec::new(), Vec::new()).unwrap();
    let image = image_node(vec![DigestInputRefV1::from_node(&layout_node)]);
    let plan = StrongDigestFinalizationPlanV1::new(vec![image, layout_node], &foundation).unwrap();

    assert_eq!(plan.nodes()[0].kind(), DigestKind::Layout);
    assert_eq!(plan.nodes()[1].kind(), DigestKind::RuntimeImage);

    let image = image_node(Vec::new());
    let image_id = image.id();
    let mismatched = image_node(vec![DigestInputRefV1::SourceSignature(image_id)]);
    assert!(matches!(
        StrongDigestFinalizationPlanV1::new(vec![mismatched], &empty_foundation()),
        Err(StrongDigestPlanBuildError::Plan(
            DigestPlanError::InputKindMismatch { .. }
        ))
    ));

    let missing =
        DigestNodeId::from_key(&DigestNodeKey::runtime_image(ConeIdentity::CORE)).unwrap();
    let image = image_node(vec![DigestInputRefV1::RuntimeImage(missing)]);
    assert!(matches!(
        StrongDigestFinalizationPlanV1::new(vec![image], &empty_foundation()),
        Err(StrongDigestPlanBuildError::Plan(
            DigestPlanError::MissingInput { .. }
        ))
    ));
}

#[test]
fn strong_profile_rejects_odr_nodes_and_requires_its_image() {
    let odr = DigestNodeV1::new(
        DigestNodeKey::odr_definition(
            scoop_identity::OdrGroupId::from_key(
                &scoop_identity::SpecializationKey::StructuralType {
                    exact_type: unit_exact_type(),
                },
            )
            .unwrap(),
        ),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        StrongDigestFinalizationPlanV1::new(vec![odr, image_node(Vec::new())], &empty_foundation()),
        Err(StrongDigestPlanBuildError::Plan(
            DigestPlanError::OdrDefinition(_)
        ))
    ));
    assert_eq!(
        StrongDigestFinalizationPlanV1::new(Vec::new(), &empty_foundation()),
        Err(StrongDigestPlanBuildError::Plan(
            DigestPlanError::MissingRuntimeImage
        ))
    );
}

#[test]
fn patch_intents_bind_one_source_to_one_real_target_atom() {
    let (foundation, target_owner, atom) = definition_foundation();
    let key = DigestNodeKey::object_definition(atom);
    let source = DigestNodeId::from_key(&key).unwrap();
    let patch = DigestPatchIntentKey::new(
        source,
        target_owner,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::DescriptorDefinition,
    );
    let definition = DigestNodeV1::new(key, Vec::new(), vec![patch]).unwrap();
    let image = image_node(vec![DigestInputRefV1::from_node(&definition)]);
    StrongDigestFinalizationPlanV1::new(vec![definition, image], &foundation).unwrap();

    let invalid = DigestPatchIntentKey::new(
        DigestNodeId::from_key(&DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE)).unwrap(),
        target_owner,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::DescriptorDefinition,
    );
    assert!(matches!(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            Vec::new(),
            vec![invalid]
        ),
        Err(DigestNodeBuildError::PatchSourceKind { .. })
    ));
}

#[test]
fn reader_rejects_noncanonical_node_order_without_sorting() {
    let (foundation, layout) = layout_foundation();
    let layout_node =
        DigestNodeV1::new(DigestNodeKey::layout(layout), Vec::new(), Vec::new()).unwrap();
    let image = image_node(vec![DigestInputRefV1::from_node(&layout_node)]);
    let plan = StrongDigestFinalizationPlanV1::new(vec![image, layout_node], &foundation).unwrap();
    let mut decoded: DecodedStrongDigestFinalizationPlanV1 =
        decode_canonical(&encode(&plan).unwrap(), DecodeLimits::default()).unwrap();
    decoded.nodes.swap(0, 1);

    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(layout).unwrap();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    assert!(matches!(
        decoded.validate(&mut identities, &foundation),
        Err(StrongDigestPlanValidationError::NonCanonicalNodeOrder { .. })
    ));
}

fn empty_foundation() -> OdrFreeLirFoundation {
    OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty())
        .unwrap()
}

fn image_node(inputs: Vec<DigestInputRefV1>) -> DigestNodeV1 {
    DigestNodeV1::new(
        DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
        inputs,
        Vec::new(),
    )
    .unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn layout_foundation() -> (OdrFreeLirFoundation, scoop_identity::PersistentLayoutId) {
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        unit_exact_type(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let id = layout.id();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_layouts(vec![layout]).unwrap();
    (
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        id,
    )
}

fn definition_foundation() -> (
    OdrFreeLirFoundation,
    scoop_identity::ObjectDefinitionPlanOwner,
    scoop_identity::ObjectDefinitionAtomId,
) {
    let exact = unit_exact_type();
    let plan_key = ObjectDefinitionPlanKey::strong(
        ConeIdentity::SINGLE_FILE,
        StrongDefinitionEntity::exact_type(exact),
        StrongDefinitionRole::TypeDescriptor,
    )
    .unwrap();
    let owner = plan_key.owner();
    let plan = CborIdentityRecord::from_key(plan_key).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let atom_id = atom.id();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan]).unwrap();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    (
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        owner,
        atom_id,
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
