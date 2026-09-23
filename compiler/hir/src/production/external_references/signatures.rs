use scoop_wire::{BudgetMeter, WirePath};

use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{
    CanonicalBinderListV1, ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority,
    NominalSourceShapeV1, TypeParameterBoundsV1,
};

pub(super) fn collect<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    collect_nominals(input, accumulator, meter)?;
    collect_callables(input, accumulator, meter)?;
    collect_properties(input, accumulator, meter)?;
    collect_source_interfaces(input, accumulator, meter)
}

fn collect_nominals<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, record) in (0_u64..).zip(input.nominal_interfaces.records()) {
        let path = WirePath::root().field(2).index(wire_index);
        collect_binders(
            record.type_parameters(),
            accumulator,
            meter,
            &path.clone().field(3),
        )?;
        for (signature_index, signature) in (0_u64..).zip(record.exact_supertypes().values()) {
            observe(
                accumulator,
                signature,
                meter,
                &path.clone().field(4).index(signature_index),
            )?;
        }
        for (field_index, field) in (0_u64..).zip(record.source_shape().declared_fields()) {
            observe(
                accumulator,
                field.value_type(),
                meter,
                &path
                    .clone()
                    .field(8)
                    .field(record.source_shape().declared_fields_wire_field())
                    .index(field_index)
                    .field(2),
            )?;
        }
        match record.source_shape() {
            NominalSourceShapeV1::Enum(shape) => {
                for (variant_index, variant) in (0_u64..).zip(shape.variants()) {
                    for (field_index, field) in (0_u64..).zip(variant.fields()) {
                        observe(
                            accumulator,
                            field.value_type(),
                            meter,
                            &path
                                .clone()
                                .field(8)
                                .field(1)
                                .index(variant_index)
                                .field(3)
                                .index(field_index)
                                .field(2),
                        )?;
                    }
                }
            }
            NominalSourceShapeV1::Struct(_)
            | NominalSourceShapeV1::Class(_)
            | NominalSourceShapeV1::Interface
            | NominalSourceShapeV1::Object(_)
            | NominalSourceShapeV1::Intrinsic(_) => continue,
        }
    }
    Ok(())
}

fn collect_callables<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, record) in (0_u64..).zip(input.callable_interfaces.records()) {
        let path = WirePath::root().field(3).index(wire_index);
        collect_binders(
            record.type_parameters(),
            accumulator,
            meter,
            &path.clone().field(3),
        )?;
        if let Some(receiver) = record.receiver() {
            observe(
                accumulator,
                receiver,
                meter,
                &path.clone().field(4).field(1),
            )?;
        }
        for (parameter_index, parameter) in (0_u64..).zip(record.parameters().parameters()) {
            observe(
                accumulator,
                parameter.value_type(),
                meter,
                &path.clone().field(5).index(parameter_index).field(2),
            )?;
        }
        observe(accumulator, record.result(), meter, &path.clone().field(6))?;
    }
    Ok(())
}

fn collect_properties<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, record) in (0_u64..).zip(input.property_interfaces.records()) {
        let path = WirePath::root().field(4).index(wire_index);
        collect_binders(
            record.type_parameters(),
            accumulator,
            meter,
            &path.clone().field(3),
        )?;
        if let Some(receiver) = record.receiver() {
            observe(
                accumulator,
                receiver,
                meter,
                &path.clone().field(4).field(1),
            )?;
        }
        observe(
            accumulator,
            record.value_type(),
            meter,
            &path.clone().field(5),
        )?;
    }
    Ok(())
}

fn collect_source_interfaces<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, record) in (0_u64..).zip(input.source_interfaces.records()) {
        let path = WirePath::root().field(6).index(wire_index);
        for (parameter_index, parameter) in (0_u64..).zip(record.parameters().parameters()) {
            let parameter_path = path.clone().field(2).index(parameter_index);
            observe(
                accumulator,
                parameter.value_type(),
                meter,
                &parameter_path.clone().field(2),
            )?;
            if let Some(element_type) = parameter.calling().element_type() {
                observe(
                    accumulator,
                    element_type,
                    meter,
                    &parameter_path.clone().field(3).field(1),
                )?;
            }
        }
    }
    Ok(())
}

fn collect_binders<A, E>(
    binders: &CanonicalBinderListV1,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (wire_index, binder) in (0_u64..).zip(binders.binders()) {
        let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() else {
            continue;
        };
        let bounds_path = path.clone().index(wire_index).field(2);
        if let Some(class) = bounds.class() {
            observe(
                accumulator,
                class,
                meter,
                &bounds_path.clone().field(1).field(1),
            )?;
        }
        for (interface_index, interface) in (0_u64..).zip(bounds.interfaces().values()) {
            observe(
                accumulator,
                interface,
                meter,
                &bounds_path.clone().field(2).index(interface_index),
            )?;
        }
    }
    Ok(())
}

fn observe<A, E>(
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    signature: &scoop_identity::SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    accumulator.observe_signature(
        signature,
        ExternalHirReferenceRoleV1::SignatureDependency,
        meter,
        path,
    )
}
