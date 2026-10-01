use super::*;

pub(super) fn check(
    input: &ConeMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    source: &CanonicalMirCallableBindingsV1,
) {
    let changed = CanonicalMirCallableBindingsV1::try_new(
        source
            .entries()
            .iter()
            .filter(|binding| {
                matches!(
                    binding.origin(),
                    scoop_mir::MirCallableOriginV1::Function(_)
                        | scoop_mir::MirCallableOriginV1::Accessor(_)
                )
            })
            .map(|binding| {
                if matches!(
                    binding.lowering_role(),
                    scoop_mir::MirCallableLoweringRoleV1::PureVirtualTrap { .. }
                ) {
                    return binding.clone();
                }
                scoop_mir::ParamFreeMirCallableBindingV1::try_new(
                    scoop_mir::MirCallableBridgeAuthority {
                        identities: graph,
                        foundation: input.foundation(),
                        types,
                    },
                    binding.origin().clone(),
                    binding.implementation(),
                    scoop_mir::MirBridgeCallableSignatureV1::new(
                        binding.semantic_signature().exact().clone(),
                        scoop_mir::GcEffect::NoGc,
                    ),
                    scoop_mir::MirBridgeCallableSignatureV1::new(
                        binding.lowered_signature().exact().clone(),
                        scoop_mir::GcEffect::NoGc,
                    ),
                    *binding.lowering_role(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(input, types, graph, types, &changed),
        Err(Error::TargetMismatch(_))
    ));
    let empty = CanonicalMirCallableBindingsV1::try_new(Vec::new()).unwrap();
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(input, types, graph, types, &empty),
        Err(Error::MissingTargetBinding(_))
    ));
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            &CanonicalParamFreeMirTypeExportsV1::default(),
            source
        ),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));

    CanonicalMirCallableBindingsV1::from_boxing_adjusts(input, types, graph, types, source)
        .unwrap();
}
