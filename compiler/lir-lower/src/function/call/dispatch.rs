//! Dynamic destinations consume their mandatory receiver separately.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum MethodDispatch {
    Virtual(u32),
    Interface(mir::InterfaceId, u32),
}

pub(super) use abi::plans::FunctionDispatch;

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
        let (receiver, dispatch) = match dispatch {
            MethodDispatch::Interface(interface, method) => {
                let (receiver, slot) = self.interface_call_receiver(receiver, interface, method);
                (receiver, MethodDispatch::Interface(interface, slot))
            }
            _ => (receiver, dispatch),
        };
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
            MethodDispatch::Interface(_, slot) => {
                let table = self.interface_component(receiver, 1);
                let receiver = self.interface_component(receiver, 0);
                let key = abi::plans::InterfaceCallable::new(self.module, callee);
                signature = self
                    .signatures
                    .plans
                    .scoop(self.signatures.requests.interfaces[&key])
                    .clone();
                (table, lir::DispatchKind::Interface, slot, receiver)
            }
        };
        let destination = self.dispatch_destination(table, kind, slot, effect);
        let args = std::iter::once(receiver).chain(arguments).collect();
        self.finish_indirect(destination, args, &signature)
    }

    fn interface_call_receiver<'e>(
        &self,
        mut receiver: &'e mir::Expr,
        mut interface: mir::InterfaceId,
        method: u32,
    ) -> (&'e mir::Expr, u32) {
        let layouts = &self.type_descriptors.interface_layouts;
        let mut slot = layouts.get(interface).methods[method as usize];
        while let mir::ExprKind::Retype { operand, .. } = &receiver.kind
            && let mir::Type::Interface(child) = operand.ty
            && let Some(offset) = layouts.projection(child, interface)
        {
            slot = slot
                .checked_add(offset)
                .expect("validated interface table offset");
            interface = child;
            receiver = operand;
        }
        (receiver, slot)
    }

    pub(super) fn lower_function_call(
        &mut self,
        dispatch: FunctionDispatch,
        function_type: mir::FunctionTypeId,
        receiver: &mir::Expr,
        arguments: &[mir::Expr],
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let key = abi::plans::FunctionExecution::new(
            self.module,
            dispatch,
            function_type,
            arguments,
            result_ty,
        );
        let signature = self
            .signatures
            .plans
            .scoop(self.signatures.requests.function_values[&key])
            .clone();
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
        let args = std::iter::once(receiver).chain(arguments).collect();
        self.finish_indirect(destination, args, &signature)
    }
}
