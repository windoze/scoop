use super::*;

pub(super) fn targets(
    context: &Context<'_>,
    owner: PersistentExactTypeId,
    ty: &mir::Type,
    schema: &hir::InheritanceSlotSchemaV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<StrongCallableDefinitionOwner>, Error> {
    let mismatch = || Error::TableMismatch {
        owner,
        role: schema.role(),
    };
    let module = context.input.module();
    let mut targets = reserve(schema.slots().len(), meter)?;
    if let mir::Type::Interface(id) = ty {
        if schema.role()
            != (hir::InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: owner,
            })
            || module.interfaces[*id].methods.len() != schema.slots().len()
        {
            return Err(mismatch());
        }
        for (slot, function) in schema.slots().iter().zip(&module.interfaces[*id].methods) {
            work(1, meter)?;
            let key = context
                .authority
                .identities
                .canonical_key::<_, DispatchSlotKey>(*slot)?;
            let target = declaration_target(key.owner());
            let binding = context.callable(target, meter)?;
            let signature = binding.lowered_signature();
            let exact = signature.exact();
            let actual = &module.functions[*function];
            work(
                (exact.parameters().len() as u64 + 1) * search(context.physical.len()),
                meter,
            )?;
            if actual.gc_effect != signature.gc_effect()
                || actual.params.len() != exact.parameters().len() + 1
                || actual.params.first().map(|parameter| &parameter.ty) != Some(ty)
                || actual
                    .params
                    .iter()
                    .skip(1)
                    .zip(exact.parameters())
                    .any(|(parameter, exact)| {
                        context.physical.get(exact).copied() != Some(&parameter.ty)
                    })
                || context.physical.get(&exact.result()).copied() != Some(&actual.return_ty)
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
            work(module.classes.len() as u64, meter)?;
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
            work(module.meta.boxed_types.len() as u64, meter)?;
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
            work(
                search(context.physical.len()) + class.itables.len() as u64,
                meter,
            )?;
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
        let mir::TableSlot::Function(function) = slot else {
            return Err(mismatch());
        };
        targets.push(context.target(*function, meter)?);
    }
    Ok(targets)
}
