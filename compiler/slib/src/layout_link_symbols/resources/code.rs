//! Prepay projection construction, plan comparisons and native contract copies.

use super::*;

pub(in super::super) fn production_projection(
    input: &ReplayInputs<'_, '_>,
    objects: &VerifiedCodeLinkObjectMemberSetV2,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let strong = input.strong;
    copy_plan(input.manifest.cone(), meter)?;
    copy_plan(strong.image_plan(), meter)?;
    copy_plan(strong.entry_plan(), meter)?;
    copy_plan(strong.digest_finalization_plan(), meter)?;
    copy_plan(strong.generated_bridge_plan(), meter)?;
    copy_plan(strong.registration_identities(), meter)?;
    let registrations = objects
        .final_objects()
        .runtime_images()
        .fingerprint()
        .registrations();
    let count = [
        registrations.safepoints().fingerprints().len(),
        registrations.callables().fingerprints().len(),
        registrations.types().fingerprints().len(),
        registrations.immortal_objects().fingerprints().len(),
        registrations.static_storages().fingerprints().len(),
        registrations.initializations().fingerprints().len(),
    ]
    .into_iter()
    .map(|count| count as u64)
    .sum::<u64>();
    table::<StrongRegistrationFingerprintEntryV1<scoop_identity::PersistentCallableBodyId>>(
        count, meter,
    )?;
    meter.charge_owned_bytes(
        std::mem::size_of::<crate::ExecutableRootProjectionV1>() as u64,
        &path,
    )?;
    meter.charge_work(
        (input.hir_foundation.as_canonical().counts().sources as u64)
            .saturating_add(input.manifest.direct_dependencies().len() as u64)
            .saturating_add(count.saturating_mul(2)),
        &path,
    )
}

pub(in super::super) fn code_contributions(
    ordinary: &CrossConeLinkSemanticImportSetV1,
    shape: &lir::CanonicalExternalShapeLinkImportsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    slots::<KnownLinkExtensionCodeContributionV1>(2, meter)?;
    copy_plan(ordinary, meter)?;
    copy_plan(shape, meter)?;
    copy_plan(&crate::lir_cross_cone_link_closure_capability(), meter)?;
    copy_plan(
        &crate::lir_cross_cone_layout_link_closure_capability(),
        meter,
    )
}

pub(in super::super) fn code_native(
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let count = native.contracts().len() as u64;
    table::<NativeExternalContractCodeRecordV1>(count, meter)?;
    for contract in native.contracts() {
        copy_plan(contract.symbol_key(), meter)?;
        copy_plan(contract.contract(), meter)?;
        meter.charge_work(
            (contract.symbol_key().native_link_symbol().as_bytes().len() as u64 + 1)
                .saturating_mul(log(count)),
            &WirePath::root(),
        )?;
    }
    Ok(())
}
