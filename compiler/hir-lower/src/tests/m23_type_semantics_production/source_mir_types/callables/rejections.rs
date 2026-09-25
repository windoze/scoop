use super::*;

pub(super) fn traps(
    output: &hir::DependencyHirOutput,
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
    unit: &CanonicalParamFreeMirTypeExportsV1,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    use scoop_mir::{
        MirCallableBridgeAuthority, MirCallableBridgeError, MirCallableLoweringRoleV1,
        ParamFreeMirCallableBindingV1,
    };
    let owners = source_dispatch::owners(output);
    let find = |owner| {
        bindings
            .entries()
            .iter()
            .find(|binding| {
                binding
                    .semantic_signature()
                    .exact()
                    .receiver()
                    .into_option()
                    == Some(owners[owner])
                    && binding.semantic_signature().exact().parameters().len() == 1
                    && matches!(
                        binding.lowering_role(),
                        MirCallableLoweringRoleV1::PureVirtualTrap { .. }
                    )
            })
            .unwrap()
    };
    let inherited = find("Again");
    let unrelated = find("Abstract");
    let index = MirTypeBridgeTypeIndexV1::try_new(&[types, unit]).unwrap();
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            MirCallableBridgeAuthority {
                identities: graph,
                foundation: input.foundation(),
                types: &index
            },
            unrelated.origin().clone(),
            unrelated.implementation(),
            unrelated.semantic_signature().clone(),
            unrelated.lowered_signature().clone(),
            *inherited.lowering_role(),
        ),
        Err(MirCallableBridgeError::InvalidTrapDeclaration)
    ));
    let partial = CanonicalParamFreeMirTypeExportsV1::try_new(
        types
            .records()
            .iter()
            .filter(|record| record.exact() != owners["Base"])
            .cloned()
            .collect(),
    )
    .unwrap();
    let index = MirTypeBridgeTypeIndexV1::try_new(&[&partial, unit]).unwrap();
    assert!(matches!(ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority { identities: graph, foundation: input.foundation(), types: &index },
        inherited.origin().clone(), inherited.implementation(), inherited.semantic_signature().clone(),
        inherited.lowered_signature().clone(), *inherited.lowering_role(),
    ), Err(MirCallableBridgeError::MissingType { exact }) if exact == owners["Base"]));
}

pub(super) fn check(
    output: &hir::DependencyHirOutput,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
) {
    let public = public_interface(output);
    assert!(matches!(
        lower_source_callable_bindings(
            output,
            &public,
            source,
            input,
            graph,
            &CanonicalParamFreeMirTypeExportsV1::default()
        ),
        Err(Error::Bridge(
            scoop_mir::MirCallableBridgeError::MissingType { .. }
        ))
    ));
    with_production("public struct Token() {}", |other, other_input, _, _, _| {
        assert!(matches!(
            lower_source_callable_bindings(other, &public, source, input, graph, types),
            Err(Error::MissingSourceMaterialization(_))
        ));
        assert!(matches!(
            lower_source_callable_bindings(output, &public, source, other_input, graph, types),
            Err(Error::MissingMirMaterialization(_))
        ));
    });

    lower_source_callable_bindings(output, &public, source, input, graph, types).unwrap();
}
