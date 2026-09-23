use super::*;
use crate::{
    CrossConeHirDeclarationValidationError, HirProductionValidatedCrossConeLayoutClosure,
    layout_compile_closure::layout_hir_semantic_closure_for_test,
};

fn prepared(bytes: &[u8]) -> HirProductionValidatedCrossConeLayoutSections<'_> {
    open_graph(bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap()
        .validate_foundation_identities([])
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_sections()
        .unwrap()
        .validate_hir_production()
        .unwrap()
}

fn closure(
    artifact: HirProductionValidatedCrossConeLayoutSections<'_>,
) -> HirProductionValidatedCrossConeLayoutClosure<'_> {
    layout_hir_semantic_closure_for_test(
        artifact.identity(),
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        vec![artifact],
        vec![Vec::new()],
    )
}

#[test]
fn ordinary_layout_declarations_replay_without_source_factories() {
    let bytes = layout_artifact(false, None);
    let mut validated = closure(prepared(&bytes))
        .validate_hir_declarations()
        .unwrap();
    assert_eq!(validated.current(), cone().identity());
    assert!(validated.direct_providers().is_empty());
    assert_eq!(validated.dependency_first().count(), 1);
    let checked = validated.validate_type_foundations().unwrap();
    assert_eq!(checked.len(), 1);
    assert!(checked[0].facts().records().is_empty());
}

#[test]
fn shared_declaration_replay_keeps_the_artifact_budget() {
    let bytes = layout_artifact(false, None);
    let mut artifact = prepared(&bytes);
    let meter = artifact.graph.envelope.meter_mut();
    let remaining = meter.limits().validation_work_units - meter.usage().validation_work_units;
    meter
        .charge_work(remaining, &scoop_wire::WirePath::root())
        .unwrap();
    let error = closure(artifact).validate_hir_declarations().err().unwrap();
    assert_eq!(error.provider, cone().identity());
    assert!(matches!(
        *error.source,
        CrossConeHirDeclarationValidationError::Resource(_)
    ));
}
