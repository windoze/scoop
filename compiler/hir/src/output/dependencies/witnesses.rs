use scoop_wire::{BudgetMeter, DecodeLimits, WireError, WirePath};

use super::*;
use crate::{ExternalHirBindingWitnessUse, ExternalHirTargetV1};

pub(super) fn collect(
    output: &crate::Output,
    selected: &crate::SelectedImportedDependencySet,
) -> Result<Vec<ExternalHirBindingWitnessUse>, DependencyCallOccurrenceError> {
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let mut uses = Vec::new();
    occurrences::visit(output, selected, &mut meter, |call, meter| {
        let target = ExternalHirTargetV1::Callable(call.callable().interface().declaration());
        append(&mut uses, target, call.binding(), meter)?;
        Ok(())
    })?;
    for constant in selected.constants() {
        let target = ExternalHirTargetV1::Property(scoop_identity::PropertyOwner::Property(
            constant.record().property(),
        ));
        append(&mut uses, target, constant.binding(), &mut meter)?;
    }
    for alias in selected.type_aliases() {
        let target = ExternalHirTargetV1::TypeAlias(alias.interface().alias());
        append(&mut uses, target, alias.binding(), &mut meter)?;
    }
    let count = uses.len() as u64;
    let route_work = uses
        .iter()
        .map(|use_| 1 + use_.witness().route().hops().len() as u64)
        .sum::<u64>();
    meter.charge_work(
        route_work.saturating_mul(2 + u64::from(count.max(1).ilog2())),
        &WirePath::root(),
    )?;
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn append(
    uses: &mut Vec<ExternalHirBindingWitnessUse>,
    target: ExternalHirTargetV1,
    binding: &crate::DirectImportedTargetBinding,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    for source in binding.sources() {
        let route = source.witness().route();
        meter.charge_work(1 + route.hops().len() as u64, &path)?;
        meter.charge_owned_bytes(
            (std::mem::size_of::<ExternalHirBindingWitnessUse>()
                + std::mem::size_of_val(route.hops())) as u64,
            &path,
        )?;
        meter.try_reserve_collection_slots(uses, 1, &path)?;
        uses.push(ExternalHirBindingWitnessUse::new(
            target,
            crate::ExternalHirBindingWitnessRole::ConcreteSelectedUse,
            source.witness().dependency().clone(),
        ));
    }
    Ok(())
}
