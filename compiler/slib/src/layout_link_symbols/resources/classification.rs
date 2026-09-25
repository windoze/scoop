use super::*;

impl SymbolCosts {
    pub(in super::super) fn current(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        bridges: &lir::GeneratedBridgePlanSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        objects.charge_strong_closure_copy(meter)?;
        copy_plan(bridges, meter)?;
        bridge_tables(bridges, meter)?;
        self.uses::<CurrentConeUndefinedRequirementUseV1>(1, meter)
    }

    pub(in super::super) fn source(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
        native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        self.copy_cross(objects, shape.legacy_closure(), meter)?;
        native::copy_native(native, meter)?;
        native::source_copies(shape, native, meter)?;
        self.uses::<SourceExternalRequirementUseV1>(3, meter)
    }

    pub(in super::super) fn runtime(
        &self,
        source: &VerifiedSourceExternalRequirementClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let registry = lir::RuntimeAbiSymbolV1::ALL.len() + lir::TargetEhSupportV1::ALL.len();
        registry_cost(registry as u64, source.target(), meter)?;
        // Each use retains one fixed runtime/EH contract and its symbol. Their
        // target strings come from the validated, fixed target selection.
        registry_cost(
            source.remaining_external_candidates().len() as u64,
            source.target(),
            meter,
        )?;
        self.uses::<RuntimeAbiRequirementUseV1>(3, meter)?;
        meter.charge_work(
            self.bindings
                .saturating_mul(self.longest_name + 1)
                .saturating_mul(log(registry as u64)),
            &WirePath::root(),
        )
    }

    pub(in super::super) fn generated(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        strong: &lir::ReplayedStrongProductionSectionV2,
        native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
        profile: &lir::CBridgeToolchainProfileV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        objects.charge_patch_site_copy(meter)?;
        copy_plan(strong.generated_bridge_plan(), meter)?;
        native::copy_native(native, meter)?;
        bridge_tables(strong.generated_bridge_plan(), meter)?;
        table::<lir::CBridgeTargetSupportRequirementV1>(
            lir::CBridgeTargetSupportV1::ALL.len() as u64,
            meter,
        )?;
        for _ in 0..=lir::CBridgeTargetSupportV1::ALL.len() {
            copy_plan(profile.id(), meter)?;
        }
        registry_cost(
            (lir::CBridgeTargetSupportV1::ALL.len() + 1) as u64,
            native.target(),
            meter,
        )?;
        let members = objects
            .patch_sites()
            .builtins()
            .strong_relocations()
            .members();
        for member in members {
            let definitions = member.definitions().definitions();
            table::<scoop_identity::ObjectDefinitionPlanId>(
                (definitions.len() as u64).saturating_mul(3),
                meter,
            )?;
        }
        self.uses::<VerifiedGeneratedBridgeRelocationSemanticUseV1>(2, meter)?;
        meter.charge_work(
            self.bindings.saturating_mul(
                log(strong.generated_bridge_plan().units().len() as u64)
                    + log(native.contracts().len() as u64),
            ),
            &WirePath::root(),
        )
    }

    pub(in super::super) fn target(
        &self,
        runtime: &VerifiedRuntimeAndEhRequirementClosureV1,
        bridges: &VerifiedGeneratedCBridgeSemanticSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        self.uses::<CBridgeTargetSupportRequirementUseV1>(8, meter)?;
        table::<crate::SlibMemberId>(
            bridges
                .scoop_patch_sites()
                .builtins()
                .member_plan()
                .generated_bridge_members()
                .len() as u64,
            meter,
        )?;
        for requirement in bridges.target_support().requirements() {
            copy_plan(requirement, meter)?;
        }
        meter.charge_work(
            (runtime.remaining_external_candidates().len() as u64)
                .saturating_mul(self.longest_name + 1)
                .saturating_mul(log(lir::CBridgeTargetSupportV1::ALL.len() as u64)),
            &WirePath::root(),
        )
    }

    pub(in super::super) fn finalization(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        cross: &VerifiedCrossConeStrongRequirementClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        self.copy_cross(objects, cross, meter)?;
        // Finalization builds ordered complete-use sets and compares every
        // field with the same Strong closure; it never drops a remainder.
        self.uses::<CanonicalUndefinedSymbolRequirementV1>(8, meter)
    }
}

fn bridge_tables(
    bridges: &lir::GeneratedBridgePlanSetV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.charge_work(bridges.units().len() as u64, &path)?;
    for plan in bridges.units() {
        let count = 1
            + plan.materialized_associated_atoms().count() as u64
            + plan.static_assert_atoms().count() as u64;
        table::<scoop_identity::GeneratedBridgeAtomId>(count.saturating_mul(6), meter)?;
        meter.charge_work(
            count.saturating_mul(log(bridges.units().len() as u64)),
            &path,
        )?;
    }
    Ok(())
}

fn registry_cost(
    count: u64,
    target: lir::LirTargetProfile,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    table::<lir::RuntimeSymbolContractV1>(count.saturating_mul(3), meter)?;
    let target = target.wire_id();
    for _ in 0..count {
        copy_plan(&target, meter)?;
    }
    // Contract hashes contain fixed ABI/target digests, a closed role, and the
    // target id charged above. Registry symbol names are fixed compiler data.
    meter.charge_owned_bytes(count.saturating_mul(256), &WirePath::root())?;
    meter.charge_work(count.saturating_mul(512), &WirePath::root())
}
