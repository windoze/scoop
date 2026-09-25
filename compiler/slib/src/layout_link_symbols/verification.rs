use super::*;

pub(crate) struct ReplayInputs<'a, 'contract> {
    pub link: &'a crate::DecodedCrossConeLayoutLinkOnlySections,
    pub foundation: &'a lir::OdrFreeLirFoundation,
    pub strong: &'a lir::ReplayedStrongProductionSectionV2,
    pub ordinary: &'a lir::CrossConeLirBridgeSectionV1,
    pub layout: &'a lir::PhysicalImportsReplayedLayoutAbiSectionV1<'contract>,
    pub selection: lir::ValidatedLirTargetSelection,
    pub profile: &'a lir::CBridgeToolchainProfileV1,
    pub manifest: &'a crate::BootstrapManifest,
    pub code_strong: &'a lir::DecodedStrongProductionSectionV2,
    pub hir_foundation: &'a scoop_hir::OdrFreeHirFoundation,
}

pub(crate) fn replay<'input, 'a, 'contract: 'a>(
    objects: ReplayedLayoutLinkObjectContentsV1<'input>,
    input: ReplayInputs<'_, '_>,
    previous: &[ReplayedLayoutLinkSymbolUsesV1<'input>],
    previous_layouts: impl Iterator<
        Item = &'a lir::PhysicalImportsReplayedLayoutAbiSectionV1<'contract>,
    >,
    reachable: &[usize],
    meter: &mut BudgetMeter,
) -> Result<ReplayedLayoutLinkSymbolUsesV1<'input>, LayoutLinkSymbolUseError> {
    let costs = resources::SymbolCosts::new(&objects, meter)?;
    let closure = objects.patch_sites().builtins().strong_relocations();
    costs.defined(closure, meter)?;
    let defined = CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(closure)?;
    let dependencies = resources::dependency_owners(previous, reachable, meter)?;
    verify_replayed_layout_strong_owners_v1(input.layout, &defined, &dependencies, meter)?;
    reject_layout_foreign_strong_owners_v1(
        input.layout.exports().provider(),
        input.layout.exports().target_profile(),
        input.layout.physical_imports(),
        previous.iter().map(|artifact| artifact.defined_symbols()),
        meter,
    )?;
    for layout in previous_layouts {
        reject_layout_foreign_strong_owners_v1(
            layout.exports().provider(),
            layout.exports().target_profile(),
            layout.physical_imports(),
            std::iter::once(&defined),
            meter,
        )?;
    }
    let native = lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation_with_meter(
        input.selection.target(),
        input.foundation,
        meter,
    )?;
    costs.dependency(
        &objects,
        input.selection.target(),
        input.strong,
        input.ordinary,
        &dependencies,
        meter,
    )?;
    let ordinary = verify_cross_cone_strong_requirements_v1(
        input.selection.target(),
        closure.clone(),
        input.strong.external_bridges().clone(),
        &dependencies,
        input.ordinary,
    )?;
    let shape = verify_replayed_external_shape_requirements_v1(&ordinary, input.layout, meter)?;
    input
        .link
        .cross_cone_link_closure_wire()
        .replay_requirements_against(&ordinary, meter)?;
    input
        .link
        .layout_link_closure_wire()
        .replay_requirements_against(&shape, meter)?;
    let undefined = requirements::complete(&objects, &input, &native, &shape, &costs, meter)?;
    input
        .link
        .link_identity_closure_wire()
        .replay_symbol_projections(&defined, undefined.legacy(), meter)?;
    let finalized = finalization::replay(&objects, &undefined, &input, &costs, meter)?;
    let (finalized, contributions) = coverage::replay(finalized, &ordinary, &shape, &input, meter)?;
    let (code, production) = code::replay(
        &finalized,
        &contributions,
        &defined,
        &native,
        &undefined,
        &input,
        meter,
    )?;
    Ok(ReplayedLayoutLinkSymbolUsesV1 {
        objects,
        defined,
        native,
        undefined,
        finalized,
        code,
        production,
    })
}
