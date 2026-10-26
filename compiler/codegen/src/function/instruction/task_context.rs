use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_task_context(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let slot = self.task_context_slot()?;
        match instruction {
            Instruction::TaskContextLoad { out } => {
                if self.function.temps[*out].ty != scoop_lir::MANAGED_PTR {
                    return Err(CodegenError(
                        "task context load requires a managed result".into(),
                    ));
                }
                let value = self
                    .builder
                    .build_load(
                        managed_ptr_ty(self.context, self.managed_address_space),
                        slot,
                        "task_context_current",
                    )
                    .map_err(context_error)?;
                self.temps.insert(*out, value);
            }
            Instruction::TaskContextStore { value } => {
                if self.function.value_ty(self.globals_arena, *value) != scoop_lir::MANAGED_PTR {
                    return Err(CodegenError(
                        "task context store requires a managed reference".into(),
                    ));
                }
                let value = self.value(*value)?;
                self.builder
                    .build_store(slot, value)
                    .map_err(context_error)?;
            }
            _ => unreachable!("task context instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}

fn context_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("emit Context access: {error}"))
}
