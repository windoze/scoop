use scoop_identity::PersistentExactTypeId;

use super::{flow::ActualType, mir};

pub(super) fn resolve(
    module: &mir::Module,
    selected: &mir::SelectedExternalMirSet,
    call: &mir::Call,
    result: &mir::Type,
    actual: ActualType,
) -> Option<(mir::Callee, mir::Type)> {
    let target = match (actual, &call.target.kind) {
        (ActualType::Class(class), mir::CallKind::Virtual { slot }) => {
            slot_target(module.classes[class].vtable.get(*slot as usize)?)?
        }
        (ActualType::Class(class), mir::CallKind::Interface { interface, slot }) => {
            let table = module.classes[class]
                .itables
                .iter()
                .find(|table| table.interface == *interface)?;
            slot_target(table.slots.get(*slot as usize)?)?
        }
        (
            ActualType::Closure(class),
            mir::CallKind::Closure { function_type }
            | mir::CallKind::FunctionBridge { function_type },
        ) => {
            let class = &module.closure_classes[class];
            let function = if class.function_type == *function_type {
                module.closure_invoke_functions[class.invoke].function
            } else {
                class
                    .bridges
                    .iter()
                    .find(|bridge| bridge.target == *function_type)?
                    .function
            };
            mir::Callee::User(function)
        }
        _ => return None,
    };
    let receiver = match target {
        mir::Callee::User(function) => {
            let function = &module.functions[function];
            if function.return_ty != *result
                || function.params.len() != call.args.len()
                || !function
                    .params
                    .iter()
                    .skip(1)
                    .zip(call.args.iter().skip(1))
                    .all(|(parameter, argument)| parameter.ty == argument.ty)
            {
                return None;
            }
            function.params.first()?.ty.clone()
        }
        mir::Callee::External(id) => {
            let external = module.meta.external_callables[id];
            let signature = selected.resolve_callable(external.reference())?.signature();
            if Some(signature.result()) != exact(module, result)
                || signature.parameters().len() + 1 != call.args.len()
                || !signature
                    .parameters()
                    .iter()
                    .zip(call.args.iter().skip(1))
                    .all(|(parameter, argument)| Some(*parameter) == exact(module, &argument.ty))
            {
                return None;
            }
            let receiver = signature.receiver().into_option()?;
            if let Some(source) = module.meta.source_exact_types.get_by_identity(receiver) {
                source.ty().clone()
            } else {
                match module
                    .meta
                    .generated_exact_types
                    .get_by_identity(receiver)?
                    .location()
                {
                    mir::GeneratedExactTypeLocation::Class(class) => mir::Type::Class(class),
                    _ => return None,
                }
            }
        }
        _ => unreachable!("dispatch tables and closure entries name ordinary implementations"),
    };
    Some((target, receiver))
}

fn slot_target(slot: &mir::TableSlot) -> Option<mir::Callee> {
    match slot {
        mir::TableSlot::Function(function) => Some(mir::Callee::User(*function)),
        mir::TableSlot::External(function) => Some(mir::Callee::External(*function)),
        mir::TableSlot::Runtime(_) => None,
    }
}

fn exact(module: &mir::Module, ty: &mir::Type) -> Option<PersistentExactTypeId> {
    if let Some(source) = module.meta.source_exact_types.get(ty) {
        return Some(source.identity_record().id());
    }
    let location = match ty {
        mir::Type::Class(class) => mir::GeneratedExactTypeLocation::Class(*class),
        mir::Type::Enum(id, _) => mir::GeneratedExactTypeLocation::Enum(*id),
        mir::Type::Context(storage) => mir::GeneratedExactTypeLocation::Context(*storage),
        _ => return None,
    };
    Some(
        module
            .meta
            .generated_exact_types
            .get(location)?
            .exact_record()
            .id(),
    )
}
