//! Charge actual object inventories and each validation pass to one artifact.

use super::*;
use scoop_wire::{WireEncode, WireError, WireErrorKind, WirePath};

mod finalization;
mod object;
use object::{ObjectCosts, inspect};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct ReplayCosts {
    members: BTreeMap<crate::SlibMemberId, ObjectCosts>,
}

impl ReplayCosts {
    pub(super) fn new(
        scoop: &[ScoopLirObjectCandidateV1<'_>],
        bridges: &[GeneratedCBridgeObjectCandidateV1<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutLinkObjectContentsError> {
        table::<(crate::SlibMemberId, ObjectCosts)>((scoop.len() + bridges.len()) as u64, meter)?;
        let mut members = BTreeMap::new();
        for (member, bytes) in scoop
            .iter()
            .map(|object| (object.member(), object.bytes()))
            .chain(
                bridges
                    .iter()
                    .map(|object| (object.member(), object.bytes())),
            )
        {
            members.insert(member, inspect(bytes, member, meter)?);
        }
        Ok(Self { members })
    }

    pub(super) fn bridge_envelopes(
        &self,
        objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
        plan: &lir::GeneratedBridgePlanSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        copy_plan(plan, meter)?;
        table::<VerifiedGeneratedCBridgeMemberEnvelopeV1>(objects.len() as u64, meter)?;
        table::<scoop_identity::GeneratedBridgeUnitId>(
            (plan.units().len() as u64).saturating_mul(4),
            meter,
        )?;
        for object in objects {
            self.members[&object.member()].envelope(meter)?;
        }
        Ok(())
    }

    pub(super) fn normalization(
        &self,
        objects: &[ScoopLirObjectCandidateV1<'_>],
        patches: usize,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        table::<VerifiedNormalizedProvisionalScoopLirObjectV1>(objects.len() as u64, meter)?;
        table::<ScoopLirObjectCandidateV1<'_>>((objects.len() as u64).saturating_mul(4), meter)?;
        table::<(u64, u64)>((patches as u64).saturating_mul(2), meter)?;
        for object in objects {
            let bytes = self.members[&object.member()].bytes;
            meter.charge_owned_bytes(bytes.saturating_mul(2), &WirePath::root())?;
            meter.charge_work(bytes.saturating_mul(2), &WirePath::root())?;
        }
        Ok(())
    }

    pub(super) fn strong(
        &self,
        symbols: &PlannedStrongObjectSymbolSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        table::<VerifiedMemberObjectRelocationIndexV1>(self.members.len() as u64, meter)?;
        for member in symbols.members() {
            self.members[&member.member()].strong(member.symbols().len() as u64, meter)?;
        }
        Ok(())
    }

    pub(super) fn copy_builtins(
        &self,
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        for costs in self.members.values() {
            costs.copy_proof(meter)?;
        }
        table::<crate::DefinitionPlanMemberAssignmentV1>(
            (builtins.member_plan().definition_assignments().len() as u64).saturating_mul(4),
            meter,
        )?;
        copy_plan(builtins.c_bridge_production().bridge_plan(), meter)?;
        copy_plan(builtins.c_bridge_production().production(), meter)
    }

    pub(super) fn digest_sites(
        &self,
        patches: &[ProvisionalDigestPatchSiteV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        table::<VerifiedMaterializedPatchSiteV1>((patches.len() as u64).saturating_mul(3), meter)?;
        for patch in patches {
            let object = self.members[&patch.member()];
            meter.charge_work(
                object.relocations.saturating_add(object.symbols),
                &WirePath::root(),
            )?;
        }
        Ok(())
    }

    pub(super) fn registrations(
        &self,
        patches: &VerifiedScoopLirDigestPatchSiteSetV1,
        count: usize,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        self.copy_builtins(patches.builtins(), meter)?;
        copy_plan(patches.digest_plan(), meter)?;
        table::<VerifiedMaterializedPatchSiteV1>(patches.sites().len() as u64, meter)?;
        // Shared registration validators search the digest nodes, patch sites
        // and actual member relocations. These products correspond to those
        // concrete lookups, never a square of unrelated table lengths.
        let digest = patches.digest_plan().nodes().len() as u64;
        let sites = patches.sites().len() as u64;
        let mut lookups = digest.saturating_add(sites).saturating_mul(8);
        for costs in self.members.values() {
            lookups = lookups.saturating_add(
                costs
                    .symbols
                    .saturating_add(costs.relocations)
                    .saturating_mul(4),
            );
            meter.charge_work(costs.bytes.saturating_mul(2), &WirePath::root())?;
        }
        meter.charge_work((count as u64).saturating_mul(lookups), &WirePath::root())?;
        table::<VerifiedStrongTypeRegistrationV1>((count as u64).saturating_mul(4), meter)
    }

    pub(super) fn stackmaps(
        &self,
        objects: &[ScoopLirObjectCandidateV1<'_>],
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
        sites: usize,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        self.copy_builtins(builtins, meter)?;
        table::<lir::StrongSafepointSemanticPlanV1>(sites as u64, meter)?;
        for object in objects {
            let costs = self.members[&object.member()];
            // Only the stackmap blob backs location/live-out tables. Ordinary
            // text and data bytes do not create stackmap records.
            table::<u64>(costs.stackmap_bytes, meter)?;
            meter.charge_work(
                (sites as u64)
                    .saturating_mul(costs.relocations.saturating_add(costs.symbols))
                    .saturating_add(costs.bytes.saturating_mul(2)),
                &WirePath::root(),
            )?;
        }
        Ok(())
    }
}

pub(super) fn table<T>(count: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
    slots::<T>(count, meter)?;
    meter.charge_work(count.saturating_mul(log(count)), &WirePath::root())
}

pub(super) fn slots<T>(count: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_table_entries(count, &path)?;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(count.saturating_mul(std::mem::size_of::<T>() as u64), &path)
}

pub(super) fn copy_plan(value: &impl WireEncode, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    let bytes = scoop_wire::cbor::encoded_length(value)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    // Typed ids, enum payloads and Vec headers have at most this logical
    // expansion over these already replayed canonical Strong plan records.
    meter.charge_owned_bytes(bytes.saturating_mul(4), &path)?;
    meter.charge_work(bytes.saturating_mul(4), &path)
}

pub(super) fn log(count: u64) -> u64 {
    1 + u64::from(count.max(1).ilog2())
}

pub(super) fn symbol_plan(
    strong: &lir::ReplayedStrongProductionSectionV2,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let plans = strong.canonical_definitions().plans();
    table::<scoop_identity::ObjectDefinitionPlanId>((plans.len() as u64).saturating_mul(2), meter)?;
    for plan in plans {
        // One primary symbol and two boundary symbols for each physical atom.
        let count = 1 + (plan.atom_boundaries().len() as u64).saturating_mul(2);
        table::<PlannedStrongObjectSymbolV1>(count.saturating_mul(3), meter)?;
    }
    copy_plan(strong.canonical_definitions(), meter)
}

pub(super) fn digest_plan(
    strong: &lir::ReplayedStrongProductionSectionV2,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let plan = strong.digest_finalization_plan();
    let nodes = plan.nodes().len() as u64;
    let definitions = strong.canonical_definitions().plans().len() as u64;
    table::<scoop_identity::DigestNodeId>(nodes.saturating_mul(4), meter)?;
    meter.charge_work(nodes.saturating_mul(definitions), &WirePath::root())?;
    for node in plan.nodes() {
        let edges = node.direct_inputs().len() as u64;
        meter.charge_edges(edges, &WirePath::root())?;
        meter.charge_work(edges.saturating_mul(log(nodes)), &WirePath::root())?;
        table::<ProvisionalDigestPatchSiteV1>(node.patch_intents().len() as u64, meter)?;
    }
    copy_plan(plan, meter)
}
