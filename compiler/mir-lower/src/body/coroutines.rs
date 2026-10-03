//! Calls to the concrete core coroutine protocols.

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_coroutine_start(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [task, completion] = args else {
            unreachable!("hir-lower validates startCoroutine's two parameters")
        };
        let result = self.lower_type(protocol.result_type);
        let task = self.lower_expr(task);
        let completion = self.lower_expr(completion);
        let callee = self.coroutine_start_callee(protocol, &result);
        self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
            mir::Type::Unit,
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args: vec![task, completion],
                return_ty: mir::Type::Unit,
            }),
        )));
        smir::Expr::unit()
    }

    pub(super) fn lower_coroutine_suspend(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [registration] = args else {
            unreachable!("hir-lower validates suspendCoroutine's one parameter")
        };
        let result = self.lower_type(protocol.result_type);
        let mut target = crate::coroutine_registry::protocol_call(
            self.module,
            self.instances,
            self.interfaces,
            protocol.suspend_registration_register,
        );
        let mir::Callee::Monomorphized(register) = target.callee else {
            unreachable!("a coroutine protocol member has a concrete application")
        };
        target.callee = mir::Callee::CoroutineSuspend { register };
        smir::Expr::new(
            result.clone(),
            smir::ExprKind::Call(smir::Call {
                target,
                args: vec![self.lower_expr(registration)],
                return_ty: result,
            }),
        )
    }

    fn coroutine_start_callee(
        &mut self,
        protocol: hir::CoroutineProtocol,
        result: &mir::Type,
    ) -> mir::Callee {
        let identity = self
            .source_exact_types
            .get(result)
            .expect("a coroutine result retains its exact identity");
        if let mir::SourceExactTypeOwner::Cone(provider) = identity.owner()
            && provider != self.module.cone
        {
            let generated = scoop_identity::PersistentGeneratedCallableId::from_key(
                &scoop_identity::GeneratedCallableKey::CoroutineStart {
                    result: identity.identity_record().id(),
                },
            )
            .expect("a concrete coroutine result has one start helper identity");
            let target =
                scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(generated);
            let callable = self
                .external_callables
                .iter()
                .find_map(|(id, callable)| {
                    (callable.reference().provider() == provider
                        && callable.reference().implementation() == target)
                        .then_some(id)
                })
                .expect("a foreign parameter-free start uses its selected provider helper");
            return mir::Callee::External(callable);
        }
        let task_interface = self.interfaces.mir_id(protocol.suspend_task);
        let continuation_interface = self.interfaces.mir_id(protocol.continuation);
        let (_, step_ty) = self.coroutines.step_for(
            self.source_exact_types,
            result,
            self.structs,
            self.enums,
            self.shell,
        );
        let protocol_call = |function| {
            crate::coroutine_registry::protocol_call(
                self.module,
                self.instances,
                self.interfaces,
                function,
            )
        };
        let run = protocol_call(protocol.suspend_task_run);
        let resume = protocol_call(protocol.continuation_resume);
        let failure = protocol_call(protocol.continuation_resume_with_exception);
        let throwable = crate::coroutine_registry::throwable_type(self.module, self.class_map);
        let helper = self.coroutines.start_helper(
            self.source_exact_types,
            result,
            task_interface,
            continuation_interface,
            run,
            resume,
            failure,
            &step_ty,
            throwable,
            self.functions,
            self.top_level,
            self.shell,
        );
        mir::Callee::User(helper)
    }
}
