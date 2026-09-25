use super::*;

#[test]
fn finite_mir_exports_require_the_same_identity_and_generated_foundation() {
    with_production(
        "public struct FiniteEmpty() {}",
        |input, graph, records, sources| {
            let empty_graph = PendingIdentityValidation::new().finish().unwrap();
            assert!(matches!(
                CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
                    input,
                    sources,
                    &empty_graph
                ),
                Err(scoop_mir::MirTypeBridgeError::Reference(_))
            ));
            let empty_foundation = scoop_mir::OdrFreeMirFoundation::try_new(
                scoop_mir::CanonicalMirFoundation::empty(),
            )
            .unwrap();
            let restored: scoop_mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(records);
            assert!(matches!(
                restored.validate(graph, &empty_foundation),
                Err(scoop_mir::MirTypeBridgeError::MissingGeneratedFoundation { .. })
            ));
        },
    );
}
