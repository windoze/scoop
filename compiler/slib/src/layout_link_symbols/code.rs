//! Close Code and the remaining manifest projection over actual replayed inputs.

use super::*;
use crate::manifest::{ProductionPlanInputs, verify_production_code_projection_common};

pub(super) fn replay(
    objects: &VerifiedCodeLinkObjectMemberSetV2,
    contributions: &CanonicalKnownLinkExtensionCodeContributionSetV1,
    defined: &CanonicalDefinedLinkSymbolOwnerSetV1,
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    undefined: &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    input: &ReplayInputs<'_>,
) -> Result<
    (
        crate::CodeFingerprint,
        crate::SingleConeProductionCodeProjectionV1,
    ),
    LayoutLinkSymbolUseError,
> {
    let manifest = input.manifest;
    let mut dependencies = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut dependencies,
        manifest.direct_dependencies().len(),
        &WirePath::root(),
    )?;
    dependencies.extend(
        manifest
            .direct_dependencies()
            .iter()
            .map(crate::DependencyRecord::identity),
    );
    let production = verify_production_code_projection_common(
        manifest.cone(),
        &dependencies,
        input
            .hir_foundation
            .source_count_for_cone(manifest.cone().identity()),
        ProductionPlanInputs::from(input.strong),
        objects,
    )?;

    let contracts = CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(native)
        .map_err(LayoutCodeFingerprintError::NativeContracts)?;
    let fingerprint = LayoutCodeFingerprintInputV1 {
        objects,
        production: &production,
        strong: input.code_strong,
        link_extension_contributions: contributions,
        native_requirements: native,
        native_contracts: &contracts,
        defined_symbols: defined,
        undefined_symbols: undefined.legacy(),
    }
    .fingerprint()?;
    if manifest.semantic_fingerprints().code()
        != crate::FingerprintAvailability::Available(fingerprint)
    {
        return Err(LayoutLinkSymbolUseError::CodeFingerprintMismatch);
    }
    input
        .link
        .production_manifest_wire()
        .replay_code_projection(
            &production,
            fingerprint,
            &contracts,
            native.library_requirements(),
        )?;
    Ok((fingerprint, production))
}
