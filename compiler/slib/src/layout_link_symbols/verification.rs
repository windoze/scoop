use super::*;

pub(crate) struct ReplayInputs<'a> {
    pub link: &'a crate::DecodedCrossConeLayoutLinkOnlySections,
    pub foundation: &'a lir::ConeLirFoundation,
    pub strong: &'a lir::ConeProductionSectionV2,
    pub ordinary: &'a lir::CrossConeLirBridgeSectionV1,
    pub layout: &'a lir::PhysicalImportsReplayedLayoutAbiSectionV1,
    pub selection: lir::ValidatedLirTargetSelection,
    pub profile: &'a lir::CBridgeToolchainProfileV1,
    pub manifest: &'a crate::BootstrapManifest,
    pub code_strong: &'a lir::DecodedConeProductionSectionV2,
    pub hir_foundation: &'a scoop_hir::OdrFreeHirFoundation,
}

pub(crate) fn replay<'a>(
    objects: ReplayedLayoutLinkObjectContentsV1,
    input: ReplayInputs<'_>,
    previous: &[ReplayedLayoutLinkSymbolUsesV1],
    previous_layouts: impl Iterator<Item = &'a lir::PhysicalImportsReplayedLayoutAbiSectionV1>,
    reachable: &[usize],
) -> Result<ReplayedLayoutLinkSymbolUsesV1, LayoutLinkSymbolUseError> {
    let closure = objects.patch_sites().builtins().strong_relocations();

    let defined = CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(closure)?;
    let dependencies = dependency_owners(previous, reachable)?;
    verify_replayed_layout_strong_owners_v1(input.layout, &defined, &dependencies)?;
    reject_layout_foreign_strong_owners_v1(
        input.layout.exports().provider(),
        input.layout.exports().target_profile(),
        input.layout.physical_imports(),
        previous.iter().map(|artifact| artifact.defined_symbols()),
    )?;
    for layout in previous_layouts {
        reject_layout_foreign_strong_owners_v1(
            layout.exports().provider(),
            layout.exports().target_profile(),
            layout.physical_imports(),
            std::iter::once(&defined),
        )?;
    }
    let native = lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        input.selection.target(),
        input.foundation,
    )?;

    let ordinary = verify_cross_cone_strong_requirements_v1(
        input.selection.target(),
        closure.clone(),
        &dependencies,
        input.ordinary,
    )?;
    let shape = verify_replayed_external_shape_requirements_v1(&ordinary, input.layout)?;
    input
        .link
        .cross_cone_link_closure_wire()
        .replay_requirements_against(&ordinary)?;
    input
        .link
        .layout_link_closure_wire()
        .replay_requirements_against(&shape)?;
    let undefined = requirements::complete(&objects, &input, &native, &shape)?;
    input
        .link
        .link_identity_closure_wire()
        .replay_symbol_projections(&defined, undefined.legacy())?;
    let finalized = finalization::replay(&objects, &undefined, &input)?;
    let (finalized, contributions) = coverage::replay(finalized, &ordinary, &shape, &input)?;
    let (code, production) = code::replay(
        &finalized,
        &contributions,
        &defined,
        &native,
        &undefined,
        &input,
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

fn dependency_owners(
    previous: &[ReplayedLayoutLinkSymbolUsesV1],
    reachable: &[usize],
) -> Result<Vec<CanonicalDefinedLinkSymbolOwnerSetV1>, WireError> {
    let path = WirePath::root();
    let mut owners = Vec::new();
    scoop_wire::allocation::try_reserve(&mut owners, reachable.len(), &path)?;

    for &index in reachable {
        let provider = previous[index].defined_symbols();
        owners.push(provider.clone());
    }
    Ok(owners)
}
