use super::*;

pub(super) fn check(
    output: &hir::DependencyHirOutput,
    input: &SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(matches!(
        lower_derived_equality_bindings(output, input, types, graph, types, &mut meter()),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    let boolean = dependencies::boolean(input, graph);
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean], &mut meter()).unwrap();
    let empty = CanonicalParamFreeMirTypeExportsV1::default();
    assert!(
        lower_derived_equality_bindings(output, input, &empty, graph, &index, &mut meter())
            .unwrap()
            .entries()
            .is_empty()
    );
    with_production("public struct Token() {}", |_, other_input, _, _, _| {
        assert!(matches!(
            lower_derived_equality_bindings(
                output,
                other_input,
                types,
                graph,
                &index,
                &mut meter()
            ),
            Err(Error::MissingMirMaterialization(_))
        ));
    });
    let mut measured = meter();
    lower_derived_equality_bindings(output, input, types, graph, &index, &mut measured).unwrap();
    let usage = measured.usage();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    lower_derived_equality_bindings(output, input, types, graph, &index, &mut shared).unwrap();
    assert!(matches!(
        lower_derived_equality_bindings(output, input, types, graph, &index, &mut shared),
        Err(Error::Resource(_))
    ));
    assert!(matches!(
        lower_derived_equality_bindings(
            output,
            input,
            types,
            graph,
            &index,
            &mut BudgetMeter::new(DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(Error::Resource(_))
    ));
}
