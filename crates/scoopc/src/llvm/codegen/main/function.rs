//! Function-level setup: callee suspend planning, return setup/finish, callee resume dispatch, codegen_top_level_fun entry.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn build_fun_callee_suspend_plan(
        &self,
        fun: &hir::FunDecl,
    ) -> Option<CalleeSuspendPlan> {
        self.build_fun_callee_suspend_plan_impl(fun)
    }

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

    #[allow(dead_code)]
    pub(crate) fn codegen_top_level_fun(
        mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = fun.body.as_ref() else {
            // extern / declaration-only：由调用点按需声明即可，这里不生成 body。
            return Ok(());
        };

        self.current_source_id = self.source_id_for_path(fun.source_path.as_path(), fun.span)?;
        self.enter_root_callable_identity(
            fun.fqn.clone(),
            self.stable_def_key_for_current_cone(
                StableDefNamespace::Fun,
                &fun.fqn,
                "top_level_fun",
            ),
        );

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let declared_return_cg = self.cg_ty_of(fun.return_ty).unwrap_or_else(|| {
            tracing::warn!(
                "codegen_top_level_fun: unsupported return type for {} -> {}",
                fun.fqn,
                self.types.display(fun.return_ty)
            );
            panic!("codegen_top_level_fun: MIR signature verifier accepted unsupported return type")
        });
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(fun.span, declared_return_cg)?
            .is_some();
        let uses_explicit_effect_hidden_abi = self
            .direct_call_abi_identity(&fun.fqn)
            .uses_effect_bridge_abi();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .unwrap_or_else(|| {
                        panic!("codegen_top_level_fun: declared sret function must publish hidden return parameter")
                    })
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.bind_explicit_effect_hidden_abi_slots(
            fun.span,
            llvm_fun,
            u32::from(uses_hidden_sret),
            uses_explicit_effect_hidden_abi,
        )?;

        self.function_cx.env.push_scope();
        self.codegen_fun_params(
            fun,
            llvm_fun,
            u32::from(uses_hidden_sret)
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi),
        )?;

        // T0141: Set up function-level return context for early return support.
        // The return slot lives in the entry block before body codegen.
        let (return_bb, return_alloca) =
            self.setup_function_return_context(fun.span, llvm_fun, declared_return_cg)?;

        let callee_suspend_plan = self.build_fun_callee_suspend_plan(fun);
        let callee_resume_entry_fn = if callee_suspend_plan.is_some() {
            Some(self.declare_top_level_fun_callee_resume_entry(fun)?)
        } else {
            None
        };
        let ret_v = if let Some(plan) = callee_suspend_plan.as_ref() {
            self.with_callee_suspend_lowering(Some(plan.clone()), callee_resume_entry_fn, |cg| {
                cg.codegen_block_as_return_value(body, declared_return_cg)
            })?
        } else {
            self.codegen_block_as_return_value(body, declared_return_cg)?
        };
        self.finish_function_return_path(fun.span, declared_return_cg, ret_v)?;

        self.emit_function_return_block(fun.span, declared_return_cg, return_bb, return_alloca)?;
        self.finish_function_explicit_frame_layout(fun.span)?;
        if let (Some(plan), Some(resume_fun)) =
            (callee_suspend_plan.as_ref(), callee_resume_entry_fn)
        {
            self.codegen_callee_resume_entry_function(
                fun.span,
                resume_fun,
                plan,
                declared_return_cg,
            )?;
        }
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        self.function_cx.env.pop_scope();
        Ok(())
    }

    // 表达式/语句/控制流 codegen 已拆分到子模块（T0102d）。
}
