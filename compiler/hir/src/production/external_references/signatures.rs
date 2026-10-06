use scoop_wire::WirePath;

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
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    collect_nominals(input, accumulator)?;
    collect_callables(input, accumulator)?;
    collect_properties(input, accumulator)?;
    collect_source_interfaces(input, accumulator)
}

fn collect_nominals<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (partition, wire_index, record) in input.nominal_interfaces.wire_records() {
        let path = WirePath::root()
            .field(2)
            .field(partition)
            .index(wire_index as u64);
        collect_binders(
            record.type_parameters(),
            accumulator,
            &path.clone().field(3),
        )?;
        for (signature_index, signature) in (0_u64..).zip(record.exact_supertypes().values()) {
            observe(
                accumulator,
                signature,
                &path.clone().field(4).index(signature_index),
            )?;
        }
        collect_dispatch_signatures(
            accumulator,
            record.declaration_details().dispatch_selections(),
            &path.clone().field(9).field(7),
        )?;
        for (field_index, field) in (0_u64..).zip(record.source_shape().declared_fields()) {
            observe(
                accumulator,
                field.value_type(),
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

fn collect_dispatch_signatures<A, E>(
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    selections: &crate::CanonicalNominalDispatchSelectionsV1,
    path: &WirePath,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (index, selection) in selections.records().iter().enumerate() {
        let path = path.clone().index(index as u64);
        observe(accumulator, selection.receiver(), &path.clone().field(3))?;
        if let crate::NominalDispatchSelectionRoleV1::Interface { interface } = selection.role() {
            observe(accumulator, interface, &path.field(0).field(1))?;
        }
    }
    Ok(())
}

fn collect_callables<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (path, record) in input
        .callable_interfaces
        .wire_declarations(WirePath::root().field(3))
    {
        collect_binders(
            record.type_parameters(),
            accumulator,
            &path.clone().field(3),
        )?;
        if let Some(receiver) = record.receiver() {
            observe(accumulator, receiver, &path.clone().field(4).field(1))?;
        }
        for (parameter_index, parameter) in (0_u64..).zip(record.parameters().parameters()) {
            observe(
                accumulator,
                parameter.value_type(),
                &path.clone().field(5).index(parameter_index).field(2),
            )?;
        }
        observe(accumulator, record.result(), &path.clone().field(6))?;
        for (index, parameter) in (0_u64..).zip(record.context_parameters()) {
            observe(
                accumulator,
                parameter.value_type(),
                &path.clone().field(11).index(index).field(2),
            )?;
        }
    }
    Ok(())
}

fn collect_properties<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (path, record) in input
        .property_interfaces
        .wire_declarations(WirePath::root().field(4))
    {
        collect_binders(
            record.type_parameters(),
            accumulator,
            &path.clone().field(3),
        )?;
        if let Some(receiver) = record.receiver() {
            observe(accumulator, receiver, &path.clone().field(4).field(1))?;
        }
        observe(accumulator, record.value_type(), &path.clone().field(5))?;
    }
    Ok(())
}

fn collect_source_interfaces<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
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
                &parameter_path.clone().field(2),
            )?;
            if let Some(element_type) = parameter.calling().element_type() {
                observe(
                    accumulator,
                    element_type,
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
            observe(accumulator, class, &bounds_path.clone().field(1).field(1))?;
        }
        for (interface_index, interface) in (0_u64..).zip(bounds.interfaces().values()) {
            observe(
                accumulator,
                interface,
                &bounds_path.clone().field(2).index(interface_index),
            )?;
        }
    }
    Ok(())
}

fn observe<A, E>(
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    signature: &scoop_identity::SignatureTypeKey,

    path: &WirePath,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    accumulator.observe_signature(
        signature,
        ExternalHirReferenceRoleV1::SignatureDependency,
        path,
    )
}
