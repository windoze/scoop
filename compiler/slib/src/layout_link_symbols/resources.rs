//! Account for the concrete tables, lookups and copies in shared classifiers.

use super::*;
use scoop_wire::{WireEncode, WireErrorKind};

mod classification;
mod dependency;
mod native;

pub(super) struct SymbolCosts {
    bindings: u64,
    names: u64,
    longest_name: u64,
}

impl SymbolCosts {
    pub(super) fn new(
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, WireError> {
        let bindings = objects
            .patch_sites()
            .builtins()
            .strong_relocations()
            .bindings();
        meter.charge_work(bindings.len() as u64, &WirePath::root())?;
        let mut costs = Self {
            bindings: bindings.len() as u64,
            names: 0,
            longest_name: 0,
        };
        for binding in bindings {
            costs.names = costs.names.saturating_add(binding.symbol().len() as u64);
            costs.longest_name = costs.longest_name.max(binding.symbol().len() as u64);
        }
        Ok(costs)
    }

    pub(super) fn defined(
        &self,
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.charge_work(closure.members().len() as u64, &path)?;
        let count = closure
            .members()
            .iter()
            .map(|member| member.definitions().symbols().len() as u64)
            .sum::<u64>();
        table::<DefinedLinkSymbolOwnerV1>(count.saturating_mul(2), meter)?;
        for member in closure.members() {
            meter.charge_work(member.definitions().symbols().len() as u64, &path)?;
            for symbol in member.definitions().symbols() {
                let length = symbol.macho_name().len() as u64;
                meter.charge_owned_bytes(length, &path)?;
                meter.charge_work((length + 1).saturating_mul(log(count) + 1), &path)?;
            }
        }
        Ok(())
    }

    pub(super) fn uses<T>(&self, copies: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
        table::<T>(self.bindings.saturating_mul(copies), meter)?;
        meter.charge_owned_bytes(self.names.saturating_mul(copies), &WirePath::root())?;
        meter.charge_work(self.names.saturating_mul(copies), &WirePath::root())
    }

    pub(super) fn copy_cross(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        cross: &VerifiedCrossConeStrongRequirementClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        objects.charge_strong_closure_copy(meter)?;
        copy_plan(cross.external_bridges(), meter)?;
        copy_plan(cross.semantic_imports(), meter)?;
        for owners in cross.dependency_owners() {
            copy_owners(owners, meter)?;
        }
        self.uses::<DependencyStrongRequirementUseV1>(3, meter)?;
        meter.charge_work(
            cross.external_requirements().len() as u64,
            &WirePath::root(),
        )?;
        for requirement in cross.external_requirements() {
            copy_plan(requirement.bridge(), meter)?;
        }
        Ok(())
    }
}

pub(super) fn dependency_owners(
    previous: &[ReplayedLayoutLinkSymbolUsesV1<'_>],
    reachable: &[usize],
    meter: &mut BudgetMeter,
) -> Result<Vec<CanonicalDefinedLinkSymbolOwnerSetV1>, WireError> {
    let path = WirePath::root();
    let mut owners = Vec::new();
    meter.try_reserve_collection_slots(&mut owners, reachable.len(), &path)?;
    meter.charge_work(reachable.len() as u64, &path)?;
    for &index in reachable {
        let provider = previous[index].defined_symbols();
        copy_owners(provider, meter)?;
        owners.push(provider.clone());
    }
    Ok(owners)
}

fn copy_owners(
    owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    slots::<DefinedLinkSymbolOwnerV1>(owners.owners().len() as u64, meter)?;
    copy_plan(owners, meter)
}

fn slots<T>(count: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_table_entries(count, &path)?;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(count.saturating_mul(std::mem::size_of::<T>() as u64), &path)
}

fn table<T>(count: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
    slots::<T>(count, meter)?;
    meter.charge_work(count.saturating_mul(log(count)), &WirePath::root())
}

fn copy_plan(value: &impl WireEncode, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    let bytes = scoop_wire::cbor::encoded_length(value)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    // Canonical plans have already passed identity and structural replay. The
    // logical copy includes enum payloads and Vec headers as well as wire data.
    meter.charge_owned_bytes(bytes.saturating_mul(4), &path)?;
    meter.charge_work(bytes.saturating_mul(4), &path)
}

fn log(count: u64) -> u64 {
    1 + u64::from(count.max(1).ilog2())
}
