//! Dynamic destinations consume their mandatory receiver separately.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum MethodDispatch {
    Virtual(u32),
    Interface(u32),
}

#[derive(Clone, Copy)]
pub(super) enum FunctionDispatch {
    Closure,
    Bridge,
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_method_call(
        &mut self,
        dispatch: MethodDispatch,
        callee: mir::DispatchCallee,
        receiver: &mir::Expr,
        arguments: &[mir::Expr],
    ) -> StorageResult<lir::Value> {
        let (mut signature, effect) = match callee {
            mir::DispatchCallee::User(id) => (
                self.function_signatures[&id].clone(),
                self.module.functions[id].gc_effect,
            ),
            mir::DispatchCallee::Monomorphized(id) => {
                let id = self.module.meta.instances[id].function;
                (
                    self.function_signatures[&id].clone(),
                    self.module.functions[id].gc_effect,
                )
            }
            mir::DispatchCallee::External(id) => {
                let callable = &self.external_callables[self.external_callable_map[&id]];
                let effect = match callable.gc_effect() {
                    lir::GcEffect::Managed => mir::GcEffect::Managed,
                    lir::GcEffect::NoGc => mir::GcEffect::NoGc,
                };
                (callable.signature().clone(), effect)
            }
        };
        assert_eq!(
            arguments.len() + 1,
            signature.logical_argument_count(),
            "method call arity"
        );
        let receiver = self.lower_expr(receiver)?;
        let arguments = arguments
            .iter()
            .map(|argument| self.lower_expr(argument))
            .collect::<StorageResult<Vec<_>>>()?;
        let (table, kind, slot, receiver) = match dispatch {
            MethodDispatch::Virtual(slot) => {
                let descriptor = self.load_at_offset(
                    receiver,
                    self.context.object_type_descriptor_offset(),
                    lir::METADATA_PTR,
                );
                let table = self.load_at_offset(
                    lir::Value::Temp(descriptor),
                    self.context.type_descriptor_vtable_offset(),
                    lir::METADATA_PTR,
                );
                (
                    lir::Value::Temp(table),
                    lir::DispatchKind::Virtual,
                    slot,
                    receiver,
                )
            }
            MethodDispatch::Interface(slot) => {
                let table = self.interface_component(receiver, 1);
                let (receiver, projected) =
                    self.project_interface_receiver(receiver, &signature)?;
                signature = projected;
                (table, lir::DispatchKind::Interface, slot, receiver)
            }
        };
        let destination = self.dispatch_destination(table, kind, slot, effect);
        let args = std::iter::once(receiver).chain(arguments).collect();
        self.finish_indirect(destination, args, &signature)
    }

    pub(super) fn lower_function_call(
        &mut self,
        dispatch: FunctionDispatch,
        function_type: mir::FunctionTypeId,
        receiver: &mir::Expr,
        arguments: &[mir::Expr],
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let signature = self.module.function_types[function_type].clone();
        let receiver_type = match dispatch {
            FunctionDispatch::Closure => mir::Type::Function(function_type),
            FunctionDispatch::Bridge => mir::Type::Any,
        };
        let source_parameter_count = signature.parameter_types.len();
        let mut parameter_types = vec![receiver_type];
        parameter_types.extend(signature.parameter_types);
        for arg in arguments.iter().skip(source_parameter_count) {
            parameter_types.push(arg.ty.clone());
        }
        assert_eq!(
            arguments.len() + 1,
            parameter_types.len(),
            "only suspend function calls add a hidden continuation argument"
        );
        let receiver = self.lower_expr(receiver)?;
        let arguments = arguments
            .iter()
            .map(|argument| self.lower_expr(argument))
            .collect::<StorageResult<Vec<_>>>()?;
        let destination = match dispatch {
            FunctionDispatch::Closure => self.managed_dispatch_destination(
                receiver,
                lir::DispatchKind::Closure,
                self.context.closure_invoke_dispatch_slot(),
            ),
            FunctionDispatch::Bridge => {
                let descriptor = self.load_at_offset(
                    receiver,
                    self.context.object_type_descriptor_offset(),
                    lir::METADATA_PTR,
                );
                let table = self.load_at_offset(
                    lir::Value::Temp(descriptor),
                    self.context.type_descriptor_vtable_offset(),
                    lir::METADATA_PTR,
                );
                self.managed_dispatch_destination(
                    lir::Value::Temp(table),
                    lir::DispatchKind::FunctionBridge,
                    0,
                )
            }
        };
        let returns_unit = !signature.is_suspend && signature.return_type == mir::Type::Unit;
        let signature = abi::classify_mir_signature(
            self.context,
            self.module,
            parameter_types.iter(),
            if returns_unit {
                &mir::Type::Unit
            } else {
                result_ty
            },
            self.structs,
            self.enums,
        )?;
        let args = std::iter::once(receiver).chain(arguments).collect();
        self.finish_indirect(destination, args, &signature)
    }
}
