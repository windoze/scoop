use super::*;

pub(super) fn targets(
    context: &Context<'_>,
    owner: PersistentExactTypeId,
    ty: &mir::Type,
    schema: &hir::InheritanceSlotSchemaV1,
) -> Result<Vec<CallableDefinitionOwner>, Error> {
    let mismatch = || Error::TableMismatch {
        owner,
        role: schema.role(),
    };
    let module = context.input.module();
    let mut targets = reserve(schema.slots().len())?;
    if let mir::Type::Interface(id) = ty {
        if schema.role()
            != (hir::InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: owner,
            })
            || module.interfaces[*id].methods.len() != schema.slots().len()
        {
            return Err(mismatch());
        }
        for (slot, actual) in schema.slots().iter().zip(&module.interfaces[*id].methods) {
            let key = context
                .authority
                .identities
                .canonical_key::<_, DispatchSlotKey>(*slot)?;
            let target = declaration_target(key.owner());
            let binding = context.callable(target)?;
            let signature = binding.lowered_signature();
            let exact = signature.exact();

            if actual.gc_effect != signature.gc_effect()
                || actual.parameters.len() != exact.parameters().len() + 1
                || actual.parameters.first() != Some(ty)
                || actual
                    .parameters
                    .iter()
                    .skip(1)
                    .zip(exact.parameters())
                    .any(|(parameter, exact)| {
                        context.physical.get(exact).copied() != Some(parameter)
                    })
                || context.physical.get(&exact.result()).copied() != Some(&actual.return_type)
            {
                return Err(mismatch());
            }
            targets.push(target);
        }
        return Ok(targets);
    }
    if *ty == mir::Type::Any && schema.slots().is_empty() {
        return Ok(targets);
    }
    let class = match ty {
        mir::Type::Class(id) => &module.classes[*id],
        mir::Type::String => {
            module
                .classes
                .iter()
                .find(|(_, class)| {
                    matches!(
                        class.representation,
                        mir::ClassRepresentation::Intrinsic(
                            mir::IntrinsicTypeRepresentation::String
                        )
                    )
                })
                .ok_or_else(mismatch)?
                .1
        }
        _ => {
            let boxed = module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| boxed.payload() == ty)
                .ok_or_else(mismatch)?;
            &module.classes[boxed.class()]
        }
    };
    let slots = match schema.role() {
        hir::InheritanceSlotSchemaRoleV1::ClassVtable => &class.vtable,
        hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
            let Some(mir::Type::Interface(interface)) =
                context.physical.get(&interface_exact).copied()
            else {
                return Err(mismatch());
            };
            &class
                .itables
                .iter()
                .find(|table| table.interface == *interface)
                .ok_or_else(mismatch)?
                .slots
        }
    };
    if slots.len() != schema.slots().len() {
        return Err(mismatch());
    }
    for slot in slots {
        targets.push(match slot {
            mir::TableSlot::Function(function) => context.target(*function)?,
            mir::TableSlot::External(callable) => context.input.module().meta.external_callables
                [*callable]
                .reference()
                .implementation()
                .into(),
            mir::TableSlot::Runtime(_) => return Err(mismatch()),
        });
    }
    Ok(targets)
}
