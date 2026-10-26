use scoop_identity::PersistentExactTypeId;

use super::{flow::ActualType, mir};

pub(super) fn direct_call(
    module: &mir::Module,
    selected: &mir::SelectedExternalMirSet,
    intrinsic: &crate::imported_bodies::IntrinsicDispatch,
    call: &mir::Call,
    result: &mir::Type,
    actual: ActualType,
) -> Option<mir::Call> {
    let (target, receiver_ty, unbox) = resolve(module, selected, intrinsic, call, result, actual)?;
    let receiver = call.target.receiver()?;
    let receiver = if unbox {
        mir::Expr::new(
            receiver_ty,
            mir::ExprKind::Unbox(Box::new(receiver.clone())),
        )
    } else if receiver.ty == receiver_ty {
        receiver.clone()
    } else {
        mir::Expr::new(
            receiver_ty.clone(),
            mir::ExprKind::Retype {
                operand: Box::new(receiver.clone()),
                ty: Box::new(receiver_ty),
            },
        )
    };
    Some(mir::Call {
        target: mir::CallTarget::Direct(target.into()),
        args: std::iter::once(receiver)
            .chain(call.args.iter().cloned())
            .collect(),
    })
}

pub(super) fn resolve(
    module: &mir::Module,
    selected: &mir::SelectedExternalMirSet,
    intrinsic: &crate::imported_bodies::IntrinsicDispatch,
    call: &mir::Call,
    result: &mir::Type,
    actual: ActualType,
) -> Option<(mir::DispatchCallee, mir::Type, bool)> {
    let semantic = match (actual, &call.target) {
        (
            ActualType::Class(class),
            mir::CallTarget::Interface {
                interface, slot, ..
            },
        ) => intrinsic.get(&(class, *interface, *slot)),
        _ => None,
    };
    let target = if let Some(target) = semantic {
        target.callee
    } else {
        match (actual, &call.target) {
            (ActualType::Class(class), mir::CallTarget::Virtual { slot, .. }) => {
                slot_target(module.classes[class].vtable.get(*slot as usize)?)?
            }
            (
                ActualType::Class(class),
                mir::CallTarget::Interface {
                    interface, slot, ..
                },
            ) => {
                let table = module.classes[class]
                    .itables
                    .iter()
                    .find(|table| table.interface == *interface)?;
                slot_target(table.slots.get(*slot as usize)?)?
            }
            (
                ActualType::Closure(class),
                mir::CallTarget::Closure { function_type, .. }
                | mir::CallTarget::FunctionBridge { function_type, .. },
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
                mir::DispatchCallee::User(function)
            }
            _ => return None,
        }
    };
    let receiver =
        match target {
            mir::DispatchCallee::User(function) => {
                local_receiver(&module.functions[function], call, result)?
            }
            mir::DispatchCallee::Monomorphized(instance) => local_receiver(
                &module.functions[module.meta.instances[instance].function],
                call,
                result,
            )?,
            mir::DispatchCallee::External(id) => {
                let external = module.meta.external_callables[id];
                let signature = selected.resolve_callable(external.reference())?.signature();
                if Some(signature.result()) != exact(module, result)
                    || signature.parameters().len() != call.args.len()
                    || !signature.parameters().iter().zip(call.args.iter()).all(
                        |(parameter, argument)| Some(*parameter) == exact(module, &argument.ty),
                    )
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
        };
    let unbox = semantic.is_some_and(|target| target.unbox);
    if unbox
        && !module
            .meta
            .boxed_types
            .iter()
            .any(|boxed| actual == ActualType::Class(boxed.class()) && boxed.payload() == &receiver)
    {
        return None;
    }
    Some((target, receiver, unbox))
}

fn slot_target(slot: &mir::TableSlot) -> Option<mir::DispatchCallee> {
    match slot {
        mir::TableSlot::Function(function) => Some(mir::DispatchCallee::User(*function)),
        mir::TableSlot::External(function) => Some(mir::DispatchCallee::External(*function)),
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

fn local_receiver(
    function: &mir::Function,
    call: &mir::Call,
    result: &mir::Type,
) -> Option<mir::Type> {
    if function.return_ty != *result
        || function.params.len() != call.args.len() + 1
        || !function
            .params
            .iter()
            .skip(1)
            .zip(&call.args)
            .all(|(parameter, argument)| parameter.ty == argument.ty)
    {
        return None;
    }
    Some(function.params.first()?.ty.clone())
}
