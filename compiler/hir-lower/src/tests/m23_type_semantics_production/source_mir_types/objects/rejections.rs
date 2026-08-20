use super::*;
use scoop_mir::{MirObjectProductionError as Error, MirTypeRepresentationV1 as Repr};

pub(super) fn check(
    input: &scoop_mir::ConeMirInput,
    graph: &mut scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    unit: &CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(matches!(
        Production::from_strong_input(input, types, graph, types),
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
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&partial, unit]).unwrap();
        let error = Production::from_strong_input(input, types, graph, &index)
            .err()
            .unwrap();
        assert!(
            matches!(error, Error::Callable(scoop_mir::MirCallableBridgeError::MissingType { exact }) | Error::Object(scoop_mir::MirObjectBridgeError::MissingType { exact }) if exact == missing.exact()),
            "{error}"
        );
    }
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, unit]).unwrap();
    with_production("public val unrelated: Int = 1", |_, other, _, _, _| {
        assert!(matches!(
            Production::from_strong_input(other, types, graph, &index),
            Err(Error::IncompleteObjects {
                expected: 1,
                actual: 0
            })
        ));
    });

    let product = Production::from_strong_input(input, types, graph, &index).unwrap();

    let decoded: scoop_mir::DecodedCanonicalMirObjectValuesV1 = decoded(product.objects());
    let empty = scoop_mir::CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
    assert!(matches!(
        decoded.validate(graph, &index, &empty),
        Err(scoop_mir::MirObjectBridgeError::MissingEnsure { .. })
    ));
}
