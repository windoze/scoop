use super::*;

pub(super) fn check(
    input: &SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    source: &CanonicalMirCallableBindingsV1,
) {
    let changed = CanonicalMirCallableBindingsV1::try_new(
        source
            .entries()
            .iter()
            .map(|binding| {
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
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            types,
            &changed,
            &mut meter()
        ),
        Err(Error::TargetMismatch(_))
    ));
    let empty = CanonicalMirCallableBindingsV1::try_new(Vec::new()).unwrap();
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            types,
            &empty,
            &mut meter()
        ),
        Err(Error::MissingTargetBinding(_))
    ));
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            &CanonicalParamFreeMirTypeExportsV1::default(),
            source,
            &mut meter()
        ),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    let mut measured = meter();
    CanonicalMirCallableBindingsV1::from_boxing_adjusts(
        input,
        types,
        graph,
        types,
        source,
        &mut measured,
    )
    .unwrap();
    let usage = measured.usage();
    assert!(usage.owned_bytes > 0 && usage.validation_work_units > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    CanonicalMirCallableBindingsV1::from_boxing_adjusts(
        input,
        types,
        graph,
        types,
        source,
        &mut shared,
    )
    .unwrap();
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            types,
            source,
            &mut shared
        ),
        Err(Error::Resource(_))
    ));
    assert!(matches!(
        CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input,
            types,
            graph,
            types,
            source,
            &mut BudgetMeter::new(DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(Error::Resource(_))
    ));
}
