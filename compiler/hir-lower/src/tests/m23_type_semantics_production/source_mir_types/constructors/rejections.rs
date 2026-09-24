use super::*;
use scoop_mir::{
    GcEffect, MirBridgeCallableSignatureV1, MirCallableBridgeAuthority, MirCallableBridgeError,
    MirCallableLoweringRoleV1, MirTypeBridgeTypeLookupV1, ParamFreeMirCallableBindingV1,
};

pub(super) fn primary_effects(
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    let authority = MirCallableBridgeAuthority {
        identities: graph,
        foundation: input.foundation(),
        types,
    };
    for binding in bindings.entries() {
        let MirCallableLoweringRoleV1::PrimaryValueConstructor { owner } = *binding.lowering_role()
        else {
            continue;
        };
        for (semantic_gc, lowered_gc, role) in [
            (GcEffect::NoGc, GcEffect::NoGc, *binding.lowering_role()),
            (
                GcEffect::Managed,
                GcEffect::Managed,
                *binding.lowering_role(),
            ),
            (
                GcEffect::Managed,
                GcEffect::NoGc,
                MirCallableLoweringRoleV1::ValueConstructor { owner },
            ),
        ] {
            assert!(matches!(
                ParamFreeMirCallableBindingV1::try_new(
                    authority,
                    binding.origin().clone(),
                    binding.implementation(),
                    MirBridgeCallableSignatureV1::new(
                        binding.semantic_signature().exact().clone(),
                        semantic_gc
                    ),
                    MirBridgeCallableSignatureV1::new(
                        binding.lowered_signature().exact().clone(),
                        lowered_gc
                    ),
                    role
                ),
                Err(MirCallableBridgeError::SignatureMismatch
                    | MirCallableBridgeError::NoGcContainsReferences { .. })
            ));
        }
    }
}

pub(super) fn secondary_is_not_field_assembly(
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    let binding = bindings
        .entries()
        .iter()
        .find(|binding| {
            matches!(
                binding.lowering_role(),
                MirCallableLoweringRoleV1::ValueConstructor { .. }
            )
        })
        .unwrap();
    assert_eq!(binding.lowered_signature().gc_effect(), GcEffect::NoGc);
    assert_eq!(binding.lowered_signature().exact().parameters().len(), 1);
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            MirCallableBridgeAuthority {
                identities: graph,
                foundation: input.foundation(),
                types
            },
            binding.origin().clone(),
            binding.implementation(),
            MirBridgeCallableSignatureV1::new(
                binding.semantic_signature().exact().clone(),
                GcEffect::Managed
            ),
            binding.lowered_signature().clone(),
            MirCallableLoweringRoleV1::PrimaryValueConstructor {
                owner: binding.semantic_signature().exact().result()
            },
        ),
        Err(MirCallableBridgeError::SignatureMismatch)
    ));
}

pub(super) fn check(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    unit: &CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(matches!(
        lower_constructor_bindings(output, public, input, graph, types, &mut meter()),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    assert!(matches!(
        lower_constructor_bindings(output, public, input, graph, unit, &mut meter()),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, unit], &mut meter()).unwrap();
    with_production(
        "public class Other public constructor()",
        |other, other_input, _, _, _| {
            assert!(matches!(
                lower_constructor_bindings(other, public, input, graph, &index, &mut meter()),
                Err(Error::IncompleteConstructors {
                    expected: 1,
                    actual: 0
                })
            ));
            assert!(matches!(
                lower_constructor_bindings(
                    output,
                    public,
                    other_input,
                    graph,
                    &index,
                    &mut meter()
                ),
                Err(Error::MissingMaterialization(_))
            ));
        },
    );
    let mut measured = meter();
    lower_constructor_bindings(output, public, input, graph, &index, &mut measured).unwrap();
    let usage = measured.usage();
    assert!(usage.validation_work_units > 0 && usage.owned_bytes > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    lower_constructor_bindings(output, public, input, graph, &index, &mut shared).unwrap();
    assert!(matches!(
        lower_constructor_bindings(output, public, input, graph, &index, &mut shared),
        Err(Error::Resource(_))
    ));
    assert!(matches!(
        lower_constructor_bindings(
            output,
            public,
            input,
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
