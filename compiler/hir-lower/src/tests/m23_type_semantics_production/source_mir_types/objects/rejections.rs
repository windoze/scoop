use super::*;
use scoop_mir::{MirObjectProductionError as Error, MirTypeRepresentationV1 as Repr};

pub(super) fn check(
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &mut scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    unit: &CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(matches!(
        Production::from_strong_input(input, types, graph, types, &mut meter()),
        Err(Error::Callable(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    for missing in types.records().iter().filter(|record| {
        matches!(
            record.representation(),
            Repr::Object { .. } | Repr::ObjectBacking { .. }
        )
    }) {
        let partial = CanonicalParamFreeMirTypeExportsV1::try_new(
            types
                .records()
                .iter()
                .filter(|record| record.exact() != missing.exact())
                .cloned()
                .collect(),
        )
        .unwrap();
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&partial, unit], &mut meter()).unwrap();
        let error = Production::from_strong_input(input, types, graph, &index, &mut meter())
            .err()
            .unwrap();
        assert!(
            matches!(error, Error::Callable(scoop_mir::MirCallableBridgeError::MissingType { exact }) | Error::Object(scoop_mir::MirObjectBridgeError::MissingType { exact }) if exact == missing.exact()),
            "{error}"
        );
    }
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, unit], &mut meter()).unwrap();
    with_production("public val unrelated: Int = 1", |_, other, _, _, _| {
        assert!(matches!(
            Production::from_strong_input(other, types, graph, &index, &mut meter()),
            Err(Error::IncompleteObjects {
                expected: 1,
                actual: 0
            })
        ));
    });
    let mut measured = meter();
    let product =
        Production::from_strong_input(input, types, graph, &index, &mut measured).unwrap();
    let usage = measured.usage();
    assert!(usage.validation_work_units > 0 && usage.owned_bytes > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    Production::from_strong_input(input, types, graph, &index, &mut shared).unwrap();
    assert!(matches!(
        Production::from_strong_input(input, types, graph, &index, &mut shared),
        Err(Error::Resource(_) | Error::Object(scoop_mir::MirObjectBridgeError::Resource(_)))
    ));
    assert!(matches!(
        Production::from_strong_input(
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
    let decoded: scoop_mir::DecodedCanonicalMirObjectValuesV1 = decoded(product.objects());
    let empty = scoop_mir::CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
    assert!(matches!(
        decoded.validate(graph, &index, &empty, &mut meter()),
        Err(scoop_mir::MirObjectBridgeError::MissingEnsure { .. })
    ));
}
