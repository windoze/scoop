//! Function-level setup: return setup/finish and callee resume dispatch.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Create the shared function-level return context used by ordinary frames.
    pub(in crate::llvm::codegen) fn setup_function_return_context(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        declared_return_cg: CgTy,
    ) -> Result<
        (
            inkwell::basic_block::BasicBlock<'ctx>,
            Option<inkwell::values::PointerValue<'ctx>>,
        ),
        LlvmEmitError,
    > {
        let return_bb = self.context.append_basic_block(llvm_fun, "return");
        let return_alloca = match declared_return_cg {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.builder.build_alloca(
                self.llvm_basic_type_of(at, declared_return_cg)?,
                "return_val",
            )?),
        };
        self.function_cx.return_context = Some(ReturnContext {
            return_bb,
            return_alloca,
        });
        Ok((return_bb, return_alloca))
    }

    /// Emit the shared return block terminator after body/resume paths branch into it.
    pub(in crate::llvm::codegen) fn emit_function_return_block(
        &mut self,
        at: crate::span::Span,
        declared_return_cg: CgTy,
        return_bb: inkwell::basic_block::BasicBlock<'ctx>,
        return_alloca: Option<inkwell::values::PointerValue<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        self.builder.position_at_end(return_bb);
        match declared_return_cg {
            CgTy::Unit => {
                self.builder.build_return(None)?;
            }
            CgTy::Never => {
                self.builder.build_unreachable()?;
            }
            _ => {
                let alloca = return_alloca.unwrap_or_else(|| {
                    panic!("emit_function_return_block: function return context must publish return alloca")
                });
                let loaded = self.builder.build_load(
                    self.llvm_basic_type_of(at, declared_return_cg)?,
                    alloca,
                    "ret_load",
                )?;
                let ret_v = self.cg_value_from_loaded(at, declared_return_cg, loaded)?;
                self.emit_return(at, declared_return_cg, ret_v)?;
            }
        }
        self.function_cx.return_context = None;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn finish_function_return_path(
        &mut self,
        at: crate::span::Span,
        declared_return_cg: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        if let Some(return_ctx) = self.function_cx.return_context {
            if let Some(alloca) = return_ctx.return_alloca
                && let Some(raw) = value.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder
                .build_unconditional_branch(return_ctx.return_bb)?;
            return Ok(());
        }

        self.emit_return(at, declared_return_cg, value)
    }

    pub(in crate::llvm::codegen) fn codegen_callee_resume_dispatch(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        base_env: &Env<'ctx>,
        declared_return_cg: CgTy,
        incoming_resume_token: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_callee_resume_dispatch_impl(
            at,
            llvm_fun,
            plan,
            base_env,
            declared_return_cg,
            incoming_resume_token,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_callee_resume_entry_function(
        &mut self,
        at: crate::span::Span,
        resume_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_callee_resume_entry_function_impl(at, resume_fun, plan, declared_return_cg)
    }

    // 表达式/语句/控制流 codegen 已拆分到子模块（T0102d）。
}
