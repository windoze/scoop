//! Ordinary callee resume and effect-call wrapper helpers.

use super::super::closure::closure_callee_resume_entry_fn_name;
use super::super::*;
impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn declare_callee_resume_entry_function_impl(
        &mut self,
        at: crate::span::Span,
        name: &str,
        return_cg: CgTy,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(name) {
            return Ok(existing);
        }

        let hidden_sret_result_ty = self.hidden_sret_result_ty(at, return_cg)?;
        let mut llvm_params = Vec::with_capacity(1 + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        // ordinary callee replay 复用 suspend-state object 作为显式 incoming token。
        llvm_params.push(self.llvm_gc_i8_ptr_type().into());

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(at, other)?
                .fn_type(&llvm_params, false),
        };

        let resume_fun = self.module.add_function(name, fn_ty, None);
        resume_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(resume_fun, 0, result_ty);
        }
        Ok(resume_fun)
    }

    pub(in crate::llvm::codegen) fn declare_top_level_fun_callee_resume_entry_impl(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "callee resume entry return type",
                at: fun.span.into(),
            })?;
        self.declare_callee_resume_entry_function(
            fun.span,
            &top_level_callee_resume_entry_fn_name(&fun.fqn),
            return_cg,
        )
    }

    pub(in crate::llvm::codegen) fn declare_closure_callee_resume_entry_impl(
        &mut self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
        return_cg: CgTy,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.declare_callee_resume_entry_function(
            at,
            &closure_callee_resume_entry_fn_name(closure.id),
            return_cg,
        )
    }

    pub(in crate::llvm::codegen) fn declare_top_level_fun_effect_call_wrapper_impl(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let wrapper_name = top_level_effect_call_wrapper_fn_name(&fun.fqn);
        if let Some(existing) = self.module.get_function(&wrapper_name) {
            return Ok(existing);
        }

        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect call wrapper return type",
                at: fun.span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(fun.span, return_cg)?;
        let ptr_ty = self.context.ptr_type(AddressSpace::default());

        let mut llvm_params =
            Vec::with_capacity(fun.params.len() + 3 + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(ptr_ty.into());
        }
        llvm_params.push(ptr_ty.into());
        llvm_params.push(self.llvm_gc_i8_ptr_type().into());
        llvm_params.push(ptr_ty.into());
        for param in &fun.params {
            llvm_params.push(
                self.ordinary_param_abi(param.span, param.ty)?
                    .llvm_param_ty(),
            );
        }

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(fun.span, other)?
                .fn_type(&llvm_params, false),
        };

        let wrapper = self.module.add_function(&wrapper_name, fn_ty, None);
        wrapper.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(wrapper, 0, result_ty);
        }
        Ok(wrapper)
    }

    pub(in crate::llvm::codegen) fn ensure_top_level_fun_effect_call_wrapper_defined_impl(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let wrapper = self.declare_top_level_fun_effect_call_wrapper(fun)?;
        if wrapper.get_first_basic_block().is_some() {
            return Ok(wrapper);
        }

        let callee_fun = self.declare_top_level_fun(fun)?;
        let saved_block = self.builder.get_insert_block();
        let mut wrapper_codegen = self.fresh_child_codegen();
        wrapper_codegen.codegen_top_level_fun_effect_call_wrapper(fun, callee_fun, wrapper)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        Ok(wrapper)
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_fun_effect_call_wrapper_impl(
        &mut self,
        fun: &hir::FunDecl,
        callee_fun: FunctionValue<'ctx>,
        wrapper_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = self.context.append_basic_block(wrapper_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(wrapper_fun)?;

        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect call wrapper return type",
                at: fun.span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(fun.span, return_cg)?;
        let mut param_index = 0u32;

        let sret_param = if hidden_sret_result_ty.is_some() {
            let value = wrapper_fun
                .get_nth_param(param_index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect call wrapper sret param",
                    at: fun.span.into(),
                })?
                .into_pointer_value();
            param_index += 1;
            Some(value)
        } else {
            None
        };

        let ctx_param = wrapper_fun
            .get_nth_param(param_index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect call wrapper ctx param",
                at: fun.span.into(),
            })?
            .into_pointer_value();
        param_index += 1;
        let incoming_resume_token_param = wrapper_fun
            .get_nth_param(param_index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect call wrapper incoming resume token param",
                at: fun.span.into(),
            })?
            .into_pointer_value();
        param_index += 1;
        let outcome_param = wrapper_fun
            .get_nth_param(param_index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect call wrapper outcome param",
                at: fun.span.into(),
            })?
            .into_pointer_value();
        param_index += 1;

        let installed_top =
            self.load_effect_ctx_handler_top_from_slot(fun.span, ctx_param, "effect_wrapper")?;
        let saved_top =
            self.swap_effect_handler_stack_top(fun.span, installed_top, "effect_wrapper_install")?;
        self.publish_incoming_resume_token(
            fun.span,
            incoming_resume_token_param,
            "effect_wrapper",
        )?;

        let mut callee_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(
                fun.params.len()
                    + usize::from(sret_param.is_some())
                    + usize::from(self.top_level_fun_uses_hidden_incoming_resume_token(fun)),
            );
        if let Some(sret_ptr) = sret_param {
            callee_args.push(sret_ptr.into());
        }
        if self.top_level_fun_uses_hidden_incoming_resume_token(fun) {
            callee_args.push(incoming_resume_token_param.into());
        }
        for offset in 0..fun.params.len() {
            let arg = wrapper_fun
                .get_nth_param(param_index + offset as u32)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect call wrapper arg",
                    at: fun.span.into(),
                })?;
            callee_args.push(arg.into());
        }

        let call_site = self
            .builder
            .build_call(callee_fun, &callee_args, "call_effect_body")?;
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_call(call_site, 0, result_ty);
        }
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(&fun.fqn));
        let deferred_direct_result = if sret_param.is_none() {
            self.defer_direct_call_result(
                fun.span,
                return_cg,
                call_site,
                "effect_wrapper_direct_result",
            )?
        } else {
            None
        };

        self.consume_current_effect_outcome_into(fun.span, outcome_param, "effect_wrapper")?;
        self.clear_incoming_resume_token(fun.span, "effect_wrapper")?;
        let _ =
            self.swap_effect_handler_stack_top(fun.span, saved_top, "effect_wrapper_restore")?;

        match return_cg {
            CgTy::Unit | CgTy::Never => {
                self.builder.build_return(None)?;
            }
            _ if sret_param.is_some() => {
                self.builder.build_return(None)?;
            }
            other => {
                let raw = self
                    .materialize_deferred_cg_value(
                        fun.span,
                        "effect_wrapper_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect call wrapper deferred return value",
                            at: fun.span.into(),
                        })?,
                    )?
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect call wrapper return value",
                        at: fun.span.into(),
                    })?;
                let _ = other;
                self.builder.build_return(Some(&raw))?;
            }
        }
        self.finish_function_explicit_frame_layout(fun.span)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn build_fun_callee_suspend_plan_impl(
        &self,
        fun: &hir::FunDecl,
    ) -> Option<CalleeSuspendPlan> {
        self.build_ordinary_callee_suspend_plan(fun.body.as_ref()?, fun.return_ty)
    }

    pub(in crate::llvm::codegen) fn codegen_callee_resume_dispatch_impl(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        base_env: &Env<'ctx>,
        declared_return_cg: CgTy,
        incoming_resume_token: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let resume_state = self.begin_callee_suspend_resume(at, plan, incoming_resume_token)?;
        let invalid_bb = self
            .context
            .append_basic_block(llvm_fun, "resume_invalid_site");
        let mut resume_site_blocks: Vec<(usize, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::with_capacity(plan.resume_sites.len());
        let mut cases = Vec::with_capacity(plan.resume_sites.len());

        for (index, site) in plan.resume_sites.iter().enumerate() {
            let bb = self
                .context
                .append_basic_block(llvm_fun, &format!("resume_site{}", site.site_tag()));
            cases.push((
                self.context
                    .i32_type()
                    .const_int(site.site_tag() as u64, false),
                bb,
            ));
            resume_site_blocks.push((index, bb));
        }

        self.builder
            .build_switch(resume_state.site_tag, invalid_bb, &cases)?;

        for (index, bb) in resume_site_blocks {
            let site = &plan.resume_sites[index];
            self.builder.position_at_end(bb);
            self.function_cx.env = base_env.clone();
            self.emit_callee_suspend_resume_site_prologue(at, plan, site, resume_state)?;
            let ret_v =
                self.with_callee_suspend_lowering(Some(plan.clone()), Some(llvm_fun), |cg| {
                    cg.codegen_block_as_return_value(&site.resume_tail, declared_return_cg)
                })?;
            self.finish_function_return_path(at, declared_return_cg, ret_v)?;
        }

        self.builder.position_at_end(invalid_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_callee_resume_entry_function_impl(
        &mut self,
        at: crate::span::Span,
        resume_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if resume_fun.get_first_basic_block().is_some() {
            return Ok(());
        }

        let saved_function_cx = self.take_function_body_cx();
        let result = (|| {
            let entry = self.context.append_basic_block(resume_fun, "entry");
            self.builder.position_at_end(entry);
            self.begin_function_explicit_frame_layout(resume_fun)?;

            self.function_cx.current_fun_return_ty = Some(declared_return_cg);
            let uses_hidden_sret = self
                .hidden_sret_result_ty(at, declared_return_cg)?
                .is_some();
            self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
                Some(
                    resume_fun
                        .get_nth_param(0)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing callee resume entry sret param",
                            at: at.into(),
                        })?
                        .into_pointer_value(),
                )
            } else {
                None
            };

            self.function_cx.env.push_scope();
            let (return_bb, return_alloca) =
                self.setup_function_return_context(at, resume_fun, declared_return_cg)?;
            let incoming_resume_token = resume_fun
                .get_nth_param(u32::from(uses_hidden_sret))
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "missing callee resume entry incoming token param",
                    at: at.into(),
                })?
                .into_pointer_value();
            self.publish_incoming_resume_token(at, incoming_resume_token, "callee_resume_entry")?;
            let base_env = self.function_cx.env.clone();

            self.codegen_callee_resume_dispatch(
                at,
                resume_fun,
                plan,
                &base_env,
                declared_return_cg,
                incoming_resume_token,
            )?;

            self.emit_function_return_block(at, declared_return_cg, return_bb, return_alloca)?;
            self.finish_function_explicit_frame_layout(at)?;
            self.function_cx.current_sret_return_ptr = None;
            self.function_cx.env.pop_scope();
            Ok(())
        })();
        self.restore_function_body_cx(saved_function_cx);
        result
    }
}
