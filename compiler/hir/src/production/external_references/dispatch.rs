use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority};

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
            .iter()
        {
            accumulator.observe(
                record.dependency_target(),
                ExternalHirReferenceRoleV1::InheritanceDependency,
            )?;
        }
    }
    Ok(())
}
