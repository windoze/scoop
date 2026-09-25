//! Costs of the shared fingerprint passes over already inspected objects.

use super::*;

impl ReplayCosts {
    pub(in super::super) fn registration_fingerprints(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        strong: &lir::ReplayedStrongProductionSectionV2,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        let count = registration_count(objects);
        // Each of the six owned leaf inputs retains its plan and patch proof.
        for _ in 0..6 {
            objects.charge_patch_site_copy(meter)?;
        }
        strong.charge_registration_plan_copies(meter)?;
        table::<VerifiedStrongTypeRegistrationV1>(count.saturating_mul(4), meter)?;
        table::<VerifiedStrongTypeFingerprintV1>(count.saturating_mul(8), meter)?;
        let bindings = objects
            .patch_sites()
            .builtins()
            .strong_relocations()
            .bindings()
            .len() as u64;
        let nodes = objects.patch_sites().digest_plan().nodes().len() as u64;
        let mut lookups = nodes.saturating_add(bindings).saturating_mul(4);
        for costs in self.members.values() {
            lookups = lookups.saturating_add(costs.relocations.saturating_add(costs.symbols));
            // Leaf and definition passes validate hashes and atom ranges from
            // the same bytes. Neither path reparses an unrelated archive.
            meter.charge_work(costs.bytes.saturating_mul(18), &path)?;
            meter.charge_owned_bytes(costs.bytes.saturating_mul(4), &path)?;
            // Safepoint and callable-body inputs each retain one stackmap copy.
            table::<u64>(costs.stackmap_bytes.saturating_mul(2), meter)?;
        }
        for _ in 0..2 {
            self.copy_builtins(objects.patch_sites().builtins(), meter)?;
        }
        meter.charge_work(count.saturating_mul(lookups), &path)?;
        table::<scoop_identity::DigestNodeId>(count.saturating_mul(12), meter)?;
        table::<ScoopLirObjectCandidateV1<'_>>(
            (objects.objects().objects().len() as u64).saturating_mul(20),
            meter,
        )?;
        // Registration patching copies the objects and reparses the envelope.
        self.object_passes(objects, 1, meter)
    }

    pub(in super::super) fn final_objects(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        strong: &lir::ReplayedStrongProductionSectionV2,
        compatibility: &crate::CompatibilityRecord,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        for _ in 0..2 {
            objects.charge_patch_site_copy(meter)?;
        }
        copy_plan(strong.image_plan(), meter)?;
        copy_plan(strong.entry_plan(), meter)?;
        copy_plan(compatibility, meter)?;
        let count = registration_count(objects);
        let nodes = strong.digest_finalization_plan().nodes().len() as u64;
        let sites = objects.patch_sites().sites().len() as u64;
        table::<scoop_identity::DigestNodeId>(count.saturating_mul(8), meter)?;
        meter.charge_work(
            count
                .saturating_mul(nodes.saturating_add(sites).saturating_add(count))
                .saturating_mul(4),
            &path,
        )?;
        // Runtime-image and entry patching each copy, normalize and validate
        // their object set. Final archive-byte equality is charged here too.
        self.object_passes(objects, 2, meter)
    }

    fn object_passes(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        passes: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        let count = objects.objects().objects().len() as u64;
        table::<VerifiedEntryPatchedScoopLirObjectV1>(
            count.saturating_mul(passes).saturating_mul(2),
            meter,
        )?;
        for object in objects.objects().objects() {
            let costs = self.members[&object.member()];
            meter
                .charge_owned_bytes(costs.bytes.saturating_mul(passes).saturating_mul(2), &path)?;
            meter.charge_work(costs.bytes.saturating_mul(passes).saturating_mul(6), &path)?;
            for _ in 0..passes {
                costs.envelope(meter)?;
            }
        }
        Ok(())
    }
}

fn registration_count(objects: &ReplayedLayoutLinkObjectContentsV1<'_>) -> u64 {
    [
        objects.safepoints().registrations().len(),
        objects.callables().registrations().len(),
        objects.types().registrations().len(),
        objects.immortals().registrations().len(),
        objects.storages().registrations().len(),
        objects.initializations().registrations().len(),
    ]
    .into_iter()
    .map(|count| count as u64)
    .sum()
}
