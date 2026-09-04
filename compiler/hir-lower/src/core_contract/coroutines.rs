use super::*;

impl Lowerer {
    pub(crate) fn validate_coroutine_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::CoroutineCore> {
        let continuation = self.require_core_interface("Continuation", files);
        let suspend_task = self.require_core_interface("SuspendTask", files);
        let suspend_registration = self.require_core_interface("SuspendRegistration", files);

        if let Some(id) = continuation {
            self.validate_continuation_contract(id);
        }
        if let Some(id) = suspend_task {
            self.validate_suspend_task_contract(id);
        }
        if let (Some(id), Some(continuation)) = (suspend_registration, continuation) {
            self.validate_suspend_registration_contract(id, continuation);
        }
        let start_coroutine =
            self.require_intrinsic(hir::IntrinsicFunctionKind::CoroutineStart, files);
        let suspend_coroutine =
            self.require_intrinsic(hir::IntrinsicFunctionKind::CoroutineSuspend, files);

        if let (Some(id), Some(continuation), Some(suspend_task)) =
            (start_coroutine, continuation, suspend_task)
        {
            self.validate_coroutine_start(id, continuation, suspend_task);
        }
        if let (Some(id), Some(suspend_registration)) = (suspend_coroutine, suspend_registration) {
            self.validate_coroutine_suspend(id, suspend_registration);
        }

        let continuation_methods = continuation
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| match methods.as_slice() {
                [resume, resume_with_exception] => Some((*resume, *resume_with_exception)),
                _ => None,
            });
        let suspend_task_run = suspend_task
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let suspend_registration_register = suspend_registration
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let (
            Some(continuation),
            Some((continuation_resume, continuation_resume_with_exception)),
            Some(suspend_task),
            Some(suspend_task_run),
            Some(suspend_registration),
            Some(suspend_registration_register),
            Some(start_coroutine),
            Some(suspend_coroutine),
        ) = (
            continuation,
            continuation_methods,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        )
        else {
            return None;
        };
        Some(hir::CoroutineCore {
            continuation,
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        })
    }

    pub(crate) fn require_core_interface(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<InterfaceId> {
        let candidate = self
            .interfaces_by_name
            .get(name)
            .map(|(id, _)| *id)
            .filter(|id| self.interface_files[id] < self.user_file_index);
        if candidate.is_none() {
            self.current_file = 0;
            self.error(
                files[0].span,
                format!("scoop.core must define interface `{name}`"),
            );
        }
        candidate
    }

    pub(crate) fn validate_continuation_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let throwable = self.throwable.map(|(_, ty)| ty);
        let valid_type_param = matches!(interface.type_params.as_slice(), [_]);
        let valid_methods = match self.interface_methods[&id].as_slice() {
            [resume, resume_exception] => {
                let resume = &self.functions[*resume];
                let resume_exception = &self.functions[*resume_exception];
                resume.name.rsplit('.').next() == Some("resume")
                    && !resume.is_suspend
                    && resume.params.len() == 2
                    && self.is_type_param(resume.params[1].ty, 0)
                    && resume.return_ty == self.unit
                    && resume_exception.name.rsplit('.').next() == Some("resumeWithException")
                    && !resume_exception.is_suspend
                    && resume_exception.params.len() == 2
                    && throwable.is_some_and(|ty| resume_exception.params[1].ty == ty)
                    && resume_exception.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_methods {
            self.error(
                interface.span,
                "interface `Continuation<T>` in scoop.core must declare exactly `fun resume(value: T)` followed by `fun resumeWithException(exception: Throwable)`"
                    .to_string(),
            );
        }
    }

    pub(crate) fn validate_suspend_task_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(interface.type_params.as_slice(), [_]);
        let valid_method = match self.interface_methods[&id].as_slice() {
            [run] => {
                let run = &self.functions[*run];
                run.name.rsplit('.').next() == Some("run")
                    && run.is_suspend
                    && run.params.len() == 1
                    && self.is_type_param(run.return_ty, 0)
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendTask<T>` in scoop.core must declare exactly `suspend fun run(): T`"
                    .to_string(),
            );
        }
    }

    pub(crate) fn validate_suspend_registration_contract(
        &mut self,
        id: InterfaceId,
        continuation: InterfaceId,
    ) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(interface.type_params.as_slice(), [_]);
        let valid_method = match self.interface_methods[&id].as_slice() {
            [register] => {
                let register = &self.functions[*register];
                register.name.rsplit('.').next() == Some("register")
                    && !register.is_suspend
                    && register.params.len() == 2
                    && self.is_interface_param(register.params[1].ty, continuation, 0)
                    && register.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendRegistration<T>` in scoop.core must declare exactly `fun register(continuation: Continuation<T>)`"
                    .to_string(),
            );
        }
    }

    pub(crate) fn validate_coroutine_start(
        &mut self,
        id: FunctionId,
        continuation: InterfaceId,
        suspend_task: InterfaceId,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "startCoroutine"
            && !sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 2
            && self.is_interface_param(sig.params[0].ty, suspend_task, 0)
            && self.is_interface_param(sig.params[1].ty, continuation, 0)
            && sig.return_ty == self.unit;
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_start` must have signature `fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>): Unit`"
                    .to_string(),
            );
        }
    }

    pub(crate) fn validate_coroutine_suspend(
        &mut self,
        id: FunctionId,
        suspend_registration: InterfaceId,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "suspendCoroutine"
            && sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 1
            && self.is_interface_param(sig.params[0].ty, suspend_registration, 0)
            && self.is_type_param(sig.return_ty, 0);
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_suspend` must have signature `suspend fun <T> suspendCoroutine(registration: SuspendRegistration<T>): T`"
                    .to_string(),
            );
        }
    }
}
