use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};

pub(super) fn collect<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for nominal in input.nominal_interfaces.all_records() {
        for record in nominal
            .declaration_details()
            .dispatch_selections()
            .records()
        {
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
