use super::*;

pub(super) fn check(
    output: &hir::DependencyHirOutput,
    input: &SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(matches!(
        lower_derived_equality_bindings(output, input, types, graph, types),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    let boolean = dependencies::boolean(input, graph);
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean]).unwrap();
    let empty = CanonicalParamFreeMirTypeExportsV1::default();
    assert!(
        lower_derived_equality_bindings(output, input, &empty, graph, &index)
            .unwrap()
            .entries()
            .is_empty()
    );
    with_production("public struct Token() {}", |_, other_input, _, _, _| {
        assert!(matches!(
            lower_derived_equality_bindings(output, other_input, types, graph, &index),
            Err(Error::MissingMirMaterialization(_))
        ));
    });

    lower_derived_equality_bindings(output, input, types, graph, &index).unwrap();
}
