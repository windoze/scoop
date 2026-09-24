use super::*;
use crate::concrete::StructConstructorKind;

pub(super) fn collect<E>(output: &mut Collector<'_>) -> Result<(), Error<E>> {
    let module = output.module;
    let values = &module.local_value_identities;
    for (id, constructor) in module.class_constructors.iter() {
        let root = constructor.materialization;
        let result = module.classes[constructor.class].canonical_type;
        output.signature(root, Part::Result, result)?;
        output.local(values.class_receiver(id).id(), result)?;
        for (index, parameter) in constructor.parameters.iter().enumerate() {
            output.signature(root, Part::Parameter(parameter_index(index)?), parameter.ty)?;
            output.local(values.class_parameter(id, index).id(), parameter.ty)?;
        }
        output
            .meter
            .charge_work(constructor.body().locals.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        for (local_id, local) in constructor.body().locals.iter() {
            if matches!(local.definition, crate::LocalValueDefinitionSite::Source(_)) {
                output.local(values.class_local(id, local_id).id(), local.ty)?;
            }
        }
    }
    for (id, constructor) in module.struct_constructors.iter() {
        let root = constructor.materialization;
        let result = module.structs[constructor.structure].canonical_type;
        output.signature(root, Part::Result, result)?;
        for (index, parameter) in constructor.parameters.iter().enumerate() {
            output.signature(root, Part::Parameter(parameter_index(index)?), parameter.ty)?;
            output.local(values.struct_parameter(id, index).id(), parameter.ty)?;
        }
        if let StructConstructorKind::Secondary {
            arguments, body, ..
        } = &constructor.kind
        {
            output
                .meter
                .charge_work(
                    (arguments.locals.len() as u64).saturating_add(body.locals.len() as u64),
                    &WirePath::root(),
                )
                .map_err(Error::Resource)?;
            let receiver = values.struct_receiver(id).ok_or(Error::MissingLocalValue)?;
            output.local(receiver.id(), result)?;
            for (local_id, local) in arguments.locals.iter() {
                if matches!(local.definition, crate::LocalValueDefinitionSite::Source(_)) {
                    let value = values
                        .struct_argument_local(id, local_id)
                        .ok_or(Error::MissingLocalValue)?;
                    output.local(value.id(), local.ty)?;
                }
            }
            for (local_id, local) in body.locals.iter() {
                if matches!(local.definition, crate::LocalValueDefinitionSite::Source(_)) {
                    let value = values
                        .struct_body_local(id, local_id)
                        .ok_or(Error::MissingLocalValue)?;
                    output.local(value.id(), local.ty)?;
                }
            }
        }
    }
    Ok(())
}
