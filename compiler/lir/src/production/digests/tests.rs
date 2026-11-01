use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DigestKind, DigestNodeId, DigestNodeKey,
    ExactTypeKey, LayoutKey, PersistentExactTypeId, RepresentationRole,
};

use super::{
    DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1, DigestPlanBuildError, DigestPlanError,
};
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn trusted_builder_sorts_nodes_and_accepts_only_typed_legal_edges() {
    let (foundation, layout) = layout_foundation();
    let layout_node = DigestNodeV1::new(DigestNodeKey::layout(layout), Vec::new()).unwrap();
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
fn lookup_uses_typed_keys_without_changing_canonical_node_order() {
    let mut layouts = Vec::new();
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let exact =
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(builtin.identity_record().id()))
                .unwrap();
        for target in [
            crate::target_fixtures::DARWIN_AARCH64_ID,
            crate::target_fixtures::LINUX_X86_64_GNU_ID,
            crate::target_fixtures::LINUX_X86_64_MUSL_ID,
        ] {
            layouts.push(
                CborIdentityRecord::from_key(LayoutKey::new(
                    exact,
                    scoop_identity::TargetProfileWireId::new(target),
                    RepresentationRole::ManagedValue,
                ))
                .unwrap(),
            );
        }
    }
    let mut nodes = layouts
        .iter()
        .map(|layout| DigestNodeV1::new(DigestNodeKey::layout(layout.id()), Vec::new()).unwrap())
        .collect::<Vec<_>>();
    nodes.push(image_node(
        nodes.iter().map(DigestInputRefV1::from_node).collect(),
    ));
    let expected = nodes.clone();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_layouts(layouts).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let plan = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();

    assert!(plan.nodes().windows(2).all(|pair| {
        (pair[0].kind().tag(), pair[0].id()) < (pair[1].kind().tag(), pair[1].id())
    }));
    assert!(
        plan.nodes()
            .windows(2)
            .any(|pair| pair[0].key() > pair[1].key())
    );
    for node in &expected {
        assert_eq!(plan.node(node.key()), Some(node));
    }
    assert_eq!(
        plan.node(&DigestNodeKey::runtime_image(ConeIdentity::CORE)),
        None
    );
}

#[test]
fn duplicate_nodes_are_rejected_before_building_the_lookup() {
    let image = image_node(Vec::new());
    let id = image.id();
    assert_eq!(
        DigestFinalizationPlanV1::new(vec![image.clone(), image], &empty_foundation()),
        Err(DigestPlanBuildError::Plan(DigestPlanError::DuplicateNode(
            id
        )))
    );
}

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

fn image_node(inputs: Vec<DigestInputRefV1>) -> DigestNodeV1 {
    DigestNodeV1::new(
        DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
        inputs,
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
