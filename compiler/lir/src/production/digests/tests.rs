use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DefinitionAtomRole, DefinitionAtomSubkey,
    DigestKind, DigestNodeId, DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole,
    ExactTypeKey, LayoutKey, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PersistentExactTypeId, RepresentationRole, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::{
    DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeBuildError, DigestNodeV1,
    DigestPlanBuildError, DigestPlanError,
};
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn trusted_builder_sorts_nodes_and_accepts_only_typed_legal_edges() {
    let (foundation, layout) = layout_foundation();
    let layout_node =
        DigestNodeV1::new(DigestNodeKey::layout(layout), Vec::new(), Vec::new()).unwrap();
    let image = image_node(vec![DigestInputRefV1::from_node(&layout_node)]);
    let plan = DigestFinalizationPlanV1::new(vec![image, layout_node], &foundation).unwrap();

    assert_eq!(plan.nodes()[0].kind(), DigestKind::Layout);
    assert_eq!(plan.nodes()[1].kind(), DigestKind::RuntimeImage);

    let image = image_node(Vec::new());
    let image_id = image.id();
    let mismatched = image_node(vec![DigestInputRefV1::SourceSignature(image_id)]);
    assert!(matches!(
        DigestFinalizationPlanV1::new(vec![mismatched], &empty_foundation()),
        Err(DigestPlanBuildError::Plan(
            DigestPlanError::InputKindMismatch { .. }
        ))
    ));

    let missing =
        DigestNodeId::from_key(&DigestNodeKey::runtime_image(ConeIdentity::CORE)).unwrap();
    let image = image_node(vec![DigestInputRefV1::RuntimeImage(missing)]);
    assert!(matches!(
        DigestFinalizationPlanV1::new(vec![image], &empty_foundation()),
        Err(DigestPlanBuildError::Plan(
            DigestPlanError::MissingInput { .. }
        ))
    ));
}

#[test]
fn digest_plan_requires_its_runtime_image() {
    assert_eq!(
        DigestFinalizationPlanV1::new(Vec::new(), &empty_foundation()),
        Err(DigestPlanBuildError::Plan(
            DigestPlanError::MissingRuntimeImage
        ))
    );
}

#[test]
fn patch_intents_bind_one_source_to_one_real_target_atom() {
    let (foundation, target_definition, atom) = definition_foundation();
    let key = DigestNodeKey::object_definition(atom);
    let source = DigestNodeId::from_key(&key).unwrap();
    let patch = DigestPatchIntentKey::new(
        source,
        target_definition,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::DescriptorDefinition,
    );
    let definition = DigestNodeV1::new(key, Vec::new(), vec![patch]).unwrap();
    let image = image_node(vec![DigestInputRefV1::from_node(&definition)]);
    DigestFinalizationPlanV1::new(vec![definition, image], &foundation).unwrap();

    let invalid = DigestPatchIntentKey::new(
        DigestNodeId::from_key(&DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE)).unwrap(),
        target_definition,
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

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
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

fn layout_foundation() -> (ConeLirFoundation, scoop_identity::PersistentLayoutId) {
    let layout = CborIdentityRecord::from_key(LayoutKey::new(
        unit_exact_type(),
        scoop_identity::TargetProfileWireId::new(crate::target_fixtures::DARWIN_AARCH64_ID),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let id = layout.id();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_layouts(vec![layout]).unwrap();
    (
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        id,
    )
}

fn definition_foundation() -> (
    ConeLirFoundation,
    scoop_identity::ObjectDefinitionPlanId,
    scoop_identity::ObjectDefinitionAtomId,
) {
    let exact = unit_exact_type();
    let plan_key = ObjectDefinitionPlanKey::strong(
        ConeIdentity::SINGLE_FILE,
        StrongDefinitionEntity::exact_type(exact),
        StrongDefinitionRole::TypeDescriptor,
    )
    .unwrap();
    let plan = CborIdentityRecord::from_key(plan_key).unwrap();
    let plan_id = plan.id();
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
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        plan_id,
        atom_id,
    )
}
