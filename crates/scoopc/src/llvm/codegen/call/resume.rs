//! Ordinary callee resume and effect-call wrapper helpers.

use super::super::closure::closure_callee_resume_entry_fn_name;
use super::super::*;
use crate::llvm::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE;

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
        resume_fun.set_gc(LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
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
            Vec::with_capacity(fun.params.len() + 2 + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(ptr_ty.into());
        }
        llvm_params.push(ptr_ty.into());
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
        wrapper.set_gc(LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
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

        let legacy_fun = self.declare_top_level_fun(fun)?;
        let saved_block = self.builder.get_insert_block();
        let mut wrapper_codegen = self.fresh_child_codegen();
        wrapper_codegen.codegen_top_level_fun_effect_call_wrapper(fun, legacy_fun, wrapper)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        Ok(wrapper)
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_fun_effect_call_wrapper_impl(
        &mut self,
        fun: &hir::FunDecl,
        legacy_fun: FunctionValue<'ctx>,
        wrapper_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = self.context.append_basic_block(wrapper_fun, "entry");
        self.builder.position_at_end(entry);

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

        let mut legacy_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(fun.params.len() + usize::from(sret_param.is_some()));
        if let Some(sret_ptr) = sret_param {
            legacy_args.push(sret_ptr.into());
        }
        for offset in 0..fun.params.len() {
            let arg = wrapper_fun
                .get_nth_param(param_index + offset as u32)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect call wrapper arg",
                    at: fun.span.into(),
                })?;
            legacy_args.push(arg.into());
        }

        let call_site = self
            .builder
            .build_call(legacy_fun, &legacy_args, "call_effect_legacy")?;
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_call(call_site, 0, result_ty);
        }
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(&fun.fqn));

        self.consume_current_effect_outcome_into(fun.span, outcome_param, "effect_wrapper")?;
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
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect call wrapper return value",
                        at: fun.span.into(),
                    },
                )?;
                let _ = other;
                self.builder.build_return(Some(&raw))?;
            }
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn build_fun_callee_suspend_plan_impl(
        &self,
        fun: &hir::FunDecl,
    ) -> Option<CalleeSuspendPlan> {
        self.build_ordinary_callee_suspend_plan_from_unified_contract(
            fun.body.as_ref()?,
            fun.return_ty,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_callee_resume_dispatch_impl(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        base_env: &Env<'ctx>,
        declared_return_cg: CgTy,
        resume_state_raw: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let resume_state = self.begin_callee_suspend_resume(at, plan, resume_state_raw)?;
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
            self.env = base_env.clone();
            self.emit_callee_suspend_resume_site_prologue(at, plan, site, resume_state)?;
            let saved_resume_entry_fn = self.current_callee_resume_entry_fn;
            self.current_callee_suspend_plan = Some(plan.clone());
            self.current_callee_resume_entry_fn = Some(llvm_fun);
            let ret_v =
                self.codegen_block_as_return_value(&site.resume_tail, declared_return_cg)?;
            self.current_callee_suspend_plan = None;
            self.current_callee_resume_entry_fn = saved_resume_entry_fn;
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

        let saved_env = std::mem::take(&mut self.env);
        let result = (|| {
            let entry = self.context.append_basic_block(resume_fun, "entry");
            self.builder.position_at_end(entry);

            self.current_fun_return_ty = Some(declared_return_cg);
            let uses_hidden_sret = self
                .hidden_sret_result_ty(at, declared_return_cg)?
                .is_some();
            self.current_sret_return_ptr = if uses_hidden_sret {
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

            self.env.push_scope();
            let (return_bb, return_alloca) =
                self.setup_function_return_context(at, resume_fun, declared_return_cg)?;
            let state_param = resume_fun
                .get_nth_param(u32::from(uses_hidden_sret))
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "missing callee resume entry state param",
                    at: at.into(),
                })?
                .into_pointer_value();
            let base_env = self.env.clone();

            self.codegen_callee_resume_dispatch(
                at,
                resume_fun,
                plan,
                &base_env,
                declared_return_cg,
                state_param,
            )?;

            self.emit_function_return_block(at, declared_return_cg, return_bb, return_alloca)?;
            self.current_sret_return_ptr = None;
            self.env.pop_scope();
            Ok(())
        })();
        self.env = saved_env;
        result
    }

    pub(in crate::llvm::codegen) fn call_callee_resume_entry_from_state_impl(
        &mut self,
        span: crate::span::Span,
        state_raw: PointerValue<'ctx>,
        result_cg: CgTy,
        label: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, result_cg)?;
        let mut llvm_param_tys =
            Vec::with_capacity(1 + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());

        let llvm_fun_ty = match (hidden_sret_result_ty, result_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let mut llvm_args = Vec::with_capacity(1 + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, &format!("{label}_sret"), result_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.push(state_raw.into());

        let resume_fn_raw = self.load_callee_suspend_resume_entry_fn_ptr(state_raw)?;
        let typed_fn_ptr = self.builder.build_pointer_cast(
            resume_fn_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            &format!("{label}_typed"),
        )?;

        let call_site = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site =
                cg.builder
                    .build_indirect_call(llvm_fun_ty, typed_fn_ptr, &llvm_args, label)?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(0);
            Ok(call_site)
        })?;

        match result_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_sret_result_from_ptr(span, result_cg, result_ptr)
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "callee resume entry return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, result_cg, raw)
                }
            }
        }
    }
}
