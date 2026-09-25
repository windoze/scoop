//! Close Code and the remaining manifest projection over actual replayed inputs.

use super::*;
use crate::manifest::{ProductionPlanInputs, verify_production_code_projection_common};

pub(super) fn replay(
    objects: &VerifiedCodeLinkObjectMemberSetV2,
    contributions: &CanonicalKnownLinkExtensionCodeContributionSetV1,
    defined: &CanonicalDefinedLinkSymbolOwnerSetV1,
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    undefined: &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    input: &ReplayInputs<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<crate::CodeFingerprint, LayoutLinkSymbolUseError> {
    resources::production_projection(input, objects, meter)?;
    let manifest = input.manifest;
    let mut dependencies = Vec::new();
    meter.try_reserve_collection_slots(
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
            .as_canonical()
            .source_count_for_cone(manifest.cone().identity()),
        ProductionPlanInputs::from(input.strong),
        objects,
    )?;
    resources::code_native(native, meter)?;
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
    .fingerprint_with_meter(meter)?;
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
            meter,
        )?;
    Ok(fingerprint)
}
