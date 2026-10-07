//! Supply the actual dependency layouts required by executable MIR fixtures.

use super::*;

pub(in crate::tests) fn lower_test_input(
    input: &mir::ConeMirInput,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let (provider_input, provider, _, _) = crate::tests::exact_callable_abi::fixture();
    let target = provider.module().meta.target_profile;
    let foundation = provider.foundation();
    let records = [mir::Type::Unit, mir::Type::Any]
        .iter()
        .map(|ty| {
            let source = provider_input
                .module()
                .meta
                .source_exact_types
                .get(ty)
                .unwrap();
            let identity = lir::ExactLayoutIdentityV1::from_foundation(
                target,
                source.identity_record().clone(),
                scoop_identity::RepresentationRole::ManagedValue,
                foundation,
            )
            .unwrap();
            if *ty == mir::Type::Unit {
                lir::ExactValueLayoutV1::unit(identity, foundation)
                    .unwrap()
                    .into()
            } else {
                lir::ExactValueLayoutV1::qualified_pointer(
                    identity,
                    lir::NichePointerKind::Managed,
                    foundation,
                )
                .unwrap()
                .into()
            }
        })
        .collect();
    let layouts = lir::CanonicalExactLayoutExportsV1::try_new(target, foundation, records).unwrap();
    let descriptors =
        lir::CanonicalExactDescriptorExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let dispatch =
        lir::CanonicalExactDispatchExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let callables =
        lir::CanonicalExactCallableAbiExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let shapes = lir::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    let ordinary =
        lir::CrossConeLirBridgeSectionV1::try_new(foundation, Vec::new(), Vec::new()).unwrap();
    let roots = layouts
        .records()
        .iter()
        .map(|record| {
            lir::LayoutAbiDependencyV1::new(
                layouts.provider(),
                lir::LayoutAbiSemanticTargetV1::Layout(record.identity().layout()),
            )
        })
        .collect::<Vec<_>>();
    let exports = lir::LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        dispatch,
        callables,
        shapes,
        ordinary,
    )
    .unwrap();
    let selected = lir::StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        target,
        &[&exports],
        Vec::new(),
        &roots,
    )
    .unwrap();
    let lowered = crate::lowering::lower_graph(
        input,
        &test_external_descriptors(input),
        &lir::SelectedExternalLirSet::empty(input.module().cone),
        target,
        Some(&selected),
    )?;
    let output = lir::ConeLirOutput::try_new(
        lowered.module,
        input
            .materialization()
            .shape_support()
            .iter()
            .map(|root| root.declaration().clone())
            .collect(),
    )
    .map_err(LirLoweringError::Output)?;
    lir::CanonicalCallableAbisV1::from_module(output.module(), output.foundation())
        .expect("every actual lowered Function has a canonical content leaf");
    Ok(output)
}
