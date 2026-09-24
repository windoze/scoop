use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};
use scoop_wire::{BudgetMeter, WirePath};

pub(super) fn collect<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (partition, index, nominal) in input.nominal_interfaces.wire_records() {
        let path = WirePath::root()
            .field(2)
            .field(partition)
            .index(index as u64)
            .field(9)
            .field(7);
        for record in nominal
            .declaration_details()
            .dispatch_selections()
            .records()
        {
            meter
                .charge_work(1, &path)
                .map_err(ExternalHirReferenceProductionError::Resource)?;
            let Some(target) = record.callable_target() else {
                continue;
            };
            accumulator.observe(
                ExternalHirTargetV1::Callable(target),
                ExternalHirReferenceRoleV1::InheritanceDependency,
            )?;
        }
    }
    Ok(())
}
