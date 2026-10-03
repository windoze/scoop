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
