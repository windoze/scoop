//! Concrete coroutine protocols use the same declaration requests as source calls.

use super::*;

mod declarations;
mod roots;

impl Concretizer<'_> {
    pub(super) fn build_coroutine_protocols(&mut self) -> Vec<concrete::CoroutineProtocol> {
        self.drain_pending_callables();
        let core = self.coroutine_declarations();
        let mut protocols = Vec::new();
        loop {
            self.drain_pending_callables();
            let mut results = self.coroutine_result_types();
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
