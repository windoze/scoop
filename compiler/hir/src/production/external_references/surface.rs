use scoop_wire::WirePath;

use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{
    DependencyBindingWitnessV1, ExportBindingSourceV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1, TypeAliasTargetV1,
};

pub(super) fn collect_reexports<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (binding_index, binding) in input.public_bindings.records().iter().enumerate() {
        let ExportBindingSourceV1::Reexport { routes } = binding.source() else {
            continue;
        };
        let target = accumulator
            .authority()
            .binding_key(binding.binding())
            .map(|key| ExternalHirTargetV1::from(key.target()))
            .ok_or(ExternalHirReferenceProductionError::MissingBindingKey {
                binding_index,
                binding: binding.binding(),
            })?;
        for route in routes.routes() {
            if !accumulator
                .add_reexport_witness(target, DependencyBindingWitnessV1::new(route.clone()))?
            {
                return Err(ExternalHirReferenceProductionError::CurrentReexportTarget {
                    binding_index,
                    target,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn collect_aliases<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, record) in (0_u64..).zip(input.type_aliases.records()) {
        let path = WirePath::root()
            .field(5)
            .index(wire_index)
            .field(2)
            .field(1);
        match record.target() {
            TypeAliasTargetV1::Alias(alias) => {
                accumulator.observe(
                    ExternalHirTargetV1::TypeAlias(*alias),
                    ExternalHirReferenceRoleV1::AliasTarget,
                )?;
            }
            TypeAliasTargetV1::Signature(signature) => accumulator.observe_signature(
                signature,
                ExternalHirReferenceRoleV1::AliasTarget,
                &path,
            )?,
        }
    }
    Ok(())
}

pub(super) fn collect_constants<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, constant) in (0_u64..).zip(input.constants.records()) {
        accumulator.observe_signature(
            constant.value_type(),
            ExternalHirReferenceRoleV1::ConstType,
            &WirePath::root().field(8).index(wire_index).field(2),
        )?;
    }
    Ok(())
}
