use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_context_access(
        &mut self,
        call: &scoop_lir::TypedCallView<'_>,
        result: &TypedCallResult<'_>,
    ) -> Result<bool, CodegenError> {
        use scoop_lir::NoGcRuntimeFunction as Leaf;
        let scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(
            operation @ (Leaf::ContextCurrent | Leaf::ContextEnter | Leaf::ContextLeave),
        )) = call.destination()
        else {
            return Ok(false);
        };
        let slot = self.task_context_slot()?;
        if let TypedCallResult::Direct { out, .. } = result {
            let previous = self
                .builder
                .build_load(
                    managed_ptr_ty(self.context, self.managed_address_space),
                    slot,
                    "task_context_previous",
                )
                .map_err(context_error)?;
            self.temps.insert(*out, previous);
        }
        if operation != Leaf::ContextCurrent {
            let task = self.value(call.args()[0].logical_value())?;
            self.builder
                .build_store(slot, task)
                .map_err(context_error)?;
        }
        Ok(true)
    }
}

fn context_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("emit Context access: {error}"))
}
