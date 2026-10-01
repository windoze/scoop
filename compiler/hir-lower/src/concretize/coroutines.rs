//! Concrete coroutine protocols use the same declaration requests as source calls.

use super::*;

mod declarations;
mod roots;

impl Concretizer<'_> {
    pub(super) fn build_coroutine_protocols(&mut self) -> Vec<concrete::CoroutineProtocol> {
        self.drain_pending_callables();
        self.seed_shared_coroutine_results();
        let core = self.coroutine_declarations();
        let mut protocols = Vec::new();
        loop {
            self.drain_pending_callables();
            let mut results = self.coroutine_results.iter().copied().collect::<Vec<_>>();
            for (index, function) in self.function_slots.iter().enumerate() {
                let Some(function) = function else {
                    continue;
                };
                if function.is_suspend
                    && matches!(
                        function.kind,
                        concrete::FunctionKind::User(_) | concrete::FunctionKind::Abstract { .. }
                    )
                {
                    results.push(function.return_ty);
                }
                if matches!(
                    function.kind,
                    concrete::FunctionKind::Intrinsic(intrinsic)
                        if matches!(
                            intrinsic.kind,
                            concrete::IntrinsicFunctionKind::CoroutineStart
                                | concrete::IntrinsicFunctionKind::CoroutineSuspend
                        )
                ) {
                    results.extend(self.function_key_arguments(&self.function_keys[index]));
                }
            }
            // Suspend function-value variance bridges are synthesized by MIR
            // and use the target function type's result. Include those
            // concrete results in HIR's closed coroutine protocol set too.
            results.extend(
                self.function_types.iter().filter_map(|(_, function)| {
                    function.is_suspend.then_some(function.return_type)
                }),
            );
            results.extend(self.interfaces.iter().flat_map(|(_, interface)| {
                interface
                    .methods
                    .iter()
                    .filter_map(|method| method.is_suspend.then_some(method.return_ty))
            }));
            results.sort_by_key(|id| id.into_raw().into_u32());
            results.dedup();
            results.retain(|result| {
                !protocols
                    .iter()
                    .any(|protocol: &concrete::CoroutineProtocol| protocol.result_type == *result)
            });
            if results.is_empty() {
                break;
            }
            if let export::CoreProtocols::Imported(core) = self.core {
                self.lower_runtime_exception_type(
                    core.exceptions().illegal_state_exception().persistent(),
                );
            }
            protocols.extend(results.into_iter().map(|result_type| {
                let continuation =
                    self.ensure_interface_definition(core.continuation, vec![result_type]);
                let suspend_task =
                    self.ensure_interface_definition(core.suspend_task, vec![result_type]);
                let suspend_registration =
                    self.ensure_interface_definition(core.suspend_registration, vec![result_type]);
                concrete::CoroutineProtocol {
                    result_type,
                    continuation,
                    suspend_task,
                    suspend_registration,
                    start_coroutine: self.request_protocol_function(
                        core.start_coroutine,
                        None,
                        result_type,
                    ),
                    suspend_coroutine: self.request_protocol_function(
                        core.suspend_coroutine,
                        None,
                        result_type,
                    ),
                    continuation_resume: self.request_protocol_function(
                        core.continuation_resume,
                        Some(concrete::MethodOwner::Interface(continuation)),
                        result_type,
                    ),
                    continuation_resume_with_exception: self.request_protocol_function(
                        core.continuation_resume_with_exception,
                        Some(concrete::MethodOwner::Interface(continuation)),
                        result_type,
                    ),
                    suspend_task_run: self.request_protocol_function(
                        core.suspend_task_run,
                        Some(concrete::MethodOwner::Interface(suspend_task)),
                        result_type,
                    ),
                    suspend_registration_register: self.request_protocol_function(
                        core.suspend_registration_register,
                        Some(concrete::MethodOwner::Interface(suspend_registration)),
                        result_type,
                    ),
                }
            }));
        }
        protocols
    }
}
