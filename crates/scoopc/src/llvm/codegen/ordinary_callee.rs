//! Ordinary callee suspend/reentry analysis and resume-entry lowering.

use std::cell::Ref;
use std::collections::HashMap;
use std::rc::Rc;

use inkwell::types::StructType;
use inkwell::values::{FunctionValue, IntValue, PointerValue};

use crate::effect::analysis::{ContinuationEscapeFacts, EffectAnalysisCtx, KnownLocalMetadata};
use crate::effect::state_machine::{
    CalleeSuspendPlan, SuspendCallAnalysis, build_ordinary_callee_suspend_plan_with_context,
    collect_known_fun_call_suspendability, function_ty_declared_effectful,
    hir_ty_is_function_value,
};
use crate::ty::TypeId;

use super::*;

#[derive(Clone, Copy)]
struct CalleeSuspendResumeState<'ctx> {
    state_ty: StructType<'ctx>,
    state_ptr: PointerValue<'ctx>,
    resume_word: IntValue<'ctx>,
    resume_gc_ref: PointerValue<'ctx>,
    site_tag: IntValue<'ctx>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn ordinary_callee_effect_analysis_ctx(&self, callable_fqn: Option<&str>) -> EffectAnalysisCtx {
        let known_fun_effects = self.known_fun_call_suspendability_map().clone();
        let mut known_local_fun_effects = HashMap::new();
        let mut known_local_metadata = HashMap::new();
        for scope in &self.function_cx.env.scopes {
            for (&id, local) in scope {
                let Some(hir_ty) = local.hir_ty else {
                    continue;
                };
                known_local_metadata.insert(
                    id,
                    KnownLocalMetadata {
                        ty: hir_ty,
                        mutable: local.mutable,
                    },
                );
                if hir_ty_is_function_value(self.types, hir_ty) {
                    known_local_fun_effects.insert(id, local.call_may_suspend);
                }
            }
        }
        let current_source = self
            .current_source()
            .expect("codegen context should always have a current source");
        EffectAnalysisCtx::new(
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            current_source.path().to_path_buf(),
            Rc::clone(&self.shared.program_facts),
        )
        .with_continuation_escape_facts(
            ContinuationEscapeFacts::from_pass_view_for_callable(
                self.materialized_pass_view(),
                callable_fqn,
                current_source.path(),
            ),
        )
    }

    fn build_ordinary_callee_suspend_plan_for_callable(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
        callable_fqn: Option<&str>,
        extra_locals: &[(hir::SymbolId, TypeId, bool)],
    ) -> Option<CalleeSuspendPlan> {
        let mut context = self.ordinary_callee_effect_analysis_ctx(callable_fqn);
        for (id, ty, mutable) in extra_locals {
            context
                .known_local_metadata
                .entry(*id)
                .or_insert(KnownLocalMetadata {
                    ty: *ty,
                    mutable: *mutable,
                });
            if hir_ty_is_function_value(self.types, *ty) {
                context
                    .known_local_fun_effects
                    .entry(*id)
                    .or_insert_with(|| self.local_call_may_suspend_from_hir_ty_impl(Some(*ty)));
            }
        }
        build_ordinary_callee_suspend_plan_with_context(
            self.types,
            body,
            declared_return_ty,
            &mut context,
        )
    }

    fn ensure_known_fun_body_may_outward_effect_cache(&self) {
        if self
            .shared_caches
            .known_fun_call_suspend_cache
            .borrow()
            .is_some()
        {
            return;
        }

        let known_fun_effects = collect_known_fun_call_suspendability(
            self.types,
            self.fun_index,
            Rc::clone(&self.shared.program_facts),
            self.materialized_pass_view(),
        );
        *self.shared_caches.known_fun_call_suspend_cache.borrow_mut() = Some(known_fun_effects);
    }

    fn known_fun_body_may_outward_effect_map(&self) -> Ref<'_, HashMap<String, bool>> {
        self.ensure_known_fun_body_may_outward_effect_cache();
        Ref::map(
            self.shared_caches.known_fun_call_suspend_cache.borrow(),
            |cache| {
                cache
                    .as_ref()
                    .expect("known fun outward-effect cache should be initialized")
            },
        )
    }

    fn known_fun_call_suspendability_map(&self) -> Ref<'_, HashMap<String, bool>> {
        self.known_fun_body_may_outward_effect_map()
    }

    fn callee_suspend_state_type_name(&self, resume_entry_fn: FunctionValue<'ctx>) -> String {
        let symbol = resume_entry_fn.get_name().to_str().unwrap_or("anon");
        format!(
            "scoop.runtime.CalleeSuspendState__{}",
            sanitize_llvm_ident(symbol)
        )
    }

    fn get_or_create_callee_suspend_state_type(
        &mut self,
        at: crate::span::Span,
        resume_entry_fn: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let type_name = self.callee_suspend_state_type_name(resume_entry_fn);
        if let Some(existing) = self.context.get_struct_type(&type_name) {
            return Ok(existing);
        }

        let ty = self.context.opaque_struct_type(&type_name);
        let mut fields = vec![
            self.llvm_gc_object_header_type().into(),
            self.context.i64_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.context.i32_type().into(),
            self.llvm_i8_ptr_type().into(),
        ];
        for local in &plan.saved_locals {
            let cg_ty = self
                .cg_ty_of(local.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee suspend local type",
                    at: at.into(),
                })?;
            fields.push(self.llvm_basic_type_of(at, cg_ty)?);
        }
        ty.set_body(&fields, false);
        Ok(ty)
    }

    fn begin_callee_suspend_resume_impl(
        &mut self,
        at: crate::span::Span,
        resume_entry_fn: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        state_raw: PointerValue<'ctx>,
    ) -> Result<CalleeSuspendResumeState<'ctx>, LlvmEmitError> {
        let state_ty = self.get_or_create_callee_suspend_state_type(at, resume_entry_fn, plan)?;
        let state_ptr = self.builder.build_pointer_cast(
            state_raw,
            self.llvm_ptr_type(self.gc_address_space()),
            "ordinary_callee_resume_state_ptr",
        )?;

        let resume_word_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            1,
            "ordinary_callee_resume_word_gep",
        )?;
        let resume_word = self
            .builder
            .build_load(
                self.context.i64_type(),
                resume_word_gep,
                "ordinary_callee_resume_word",
            )?
            .into_int_value();

        let resume_gc_ref_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            2,
            "ordinary_callee_resume_gc_ref_gep",
        )?;
        let resume_gc_ref = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                resume_gc_ref_gep,
                "ordinary_callee_resume_gc_ref",
            )?
            .into_pointer_value();

        let site_tag_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            3,
            "ordinary_callee_resume_site_tag_gep",
        )?;
        let site_tag = self
            .builder
            .build_load(
                self.context.i32_type(),
                site_tag_gep,
                "ordinary_callee_resume_site_tag",
            )?
            .into_int_value();

        Ok(CalleeSuspendResumeState {
            state_ty,
            state_ptr,
            resume_word,
            resume_gc_ref,
            site_tag,
        })
    }

    fn callee_suspend_saved_local_field_index(
        &self,
        plan: &CalleeSuspendPlan,
        local_id: hir::SymbolId,
    ) -> Option<u32> {
        plan.saved_local_index(local_id)
            .map(|index| CALLEE_SUSPEND_STATE_USER_FIELD_BASE_INDEX + index)
    }

    fn emit_callee_suspend_resume_site_prologue_impl(
        &mut self,
        at: crate::span::Span,
        plan: &CalleeSuspendPlan,
        site_index: usize,
        resume_state: CalleeSuspendResumeState<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let site = &plan.resume_sites[site_index];
        for local_plan in &site.saved_locals {
            let cg_ty = self
                .cg_ty_of(local_plan.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume local type",
                    at: at.into(),
                })?;
            let field_index = self
                .callee_suspend_saved_local_field_index(plan, local_plan.id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume local field index",
                    at: at.into(),
                })?;
            let field_ptr = self.builder.build_struct_gep(
                resume_state.state_ty,
                resume_state.state_ptr,
                field_index,
                &format!("callee_resume_local_{}", local_plan.id.as_u32()),
            )?;
            let restored = match cg_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => {
                    let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        field_ptr,
                        &format!("callee_resume_load_{}", local_plan.id.as_u32()),
                    )?;
                    self.cg_value_from_loaded(at, cg_ty, loaded)?
                }
            };
            let name = if local_plan.name.is_empty() {
                format!("resumed_local_{}", local_plan.id.as_u32())
            } else {
                format!("resumed_{}", local_plan.name)
            };
            let ptr = self.create_entry_alloca(at, &name, cg_ty)?;
            let _ = self.store_local_value(at, ptr, cg_ty, restored)?;
            self.function_cx.env.insert(
                local_plan.id,
                CgLocal {
                    hir_ty: Some(local_plan.ty),
                    call_may_suspend: self
                        .local_call_may_suspend_from_hir_ty_impl(Some(local_plan.ty)),
                    ty: cg_ty,
                    ptr,
                    frame_backing_ptr: None,
                    mutable: local_plan.mutable,
                },
            );
        }

        let resume_slot_cg_ty =
            self.cg_ty_of(site.resume_slot_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume slot type",
                    at: at.into(),
                })?;
        let resume_slot_value = self.decode_effect_transport_value(
            at,
            resume_state.resume_word,
            resume_state.resume_gc_ref,
            resume_slot_cg_ty,
        )?;
        let resume_slot_name = if site.resume_slot_name.is_empty() {
            format!("callee_resume_slot_{}", site.resume_slot_id.as_u32())
        } else {
            format!("resumed_{}", site.resume_slot_name)
        };
        let resume_slot_ptr = self.create_entry_alloca(at, &resume_slot_name, resume_slot_cg_ty)?;
        let _ =
            self.store_local_value(at, resume_slot_ptr, resume_slot_cg_ty, resume_slot_value)?;
        self.function_cx.env.insert(
            site.resume_slot_id,
            CgLocal {
                hir_ty: Some(site.resume_slot_ty),
                call_may_suspend: self
                    .local_call_may_suspend_from_hir_ty_impl(Some(site.resume_slot_ty)),
                ty: resume_slot_cg_ty,
                ptr: resume_slot_ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
        );
        Ok(())
    }

    pub(in crate::llvm::codegen) fn build_fun_callee_suspend_plan_impl(
        &self,
        fun: &hir::FunDecl,
    ) -> Option<CalleeSuspendPlan> {
        if !self.callable_needs_callee_resume_shell(&fun.fqn) {
            return None;
        }
        self.build_ordinary_callee_suspend_plan_for_callable(
            fun.body.as_ref()?,
            fun.return_ty,
            Some(fun.fqn.as_str()),
            &[],
        )
    }

    pub(in crate::llvm::codegen) fn build_closure_callee_suspend_plan_impl(
        &self,
        closure: &hir::ClosureExpr,
        return_ty: TypeId,
        receiver_binding: Option<&(hir::SymbolId, String, TypeId)>,
        param_bindings: &[(hir::SymbolId, String, TypeId)],
    ) -> Option<CalleeSuspendPlan> {
        let callable_fqn = format!("scoop.lambda${}", closure.id.as_u32());
        if !self.callable_needs_callee_resume_shell(&callable_fqn) {
            return None;
        }
        let hir::ExprKind::Block(block) = &closure.body.kind else {
            return None;
        };

        let mut extra_locals =
            Vec::with_capacity(param_bindings.len() + usize::from(receiver_binding.is_some()));
        if let Some((id, _name, ty)) = receiver_binding {
            extra_locals.push((*id, *ty, false));
        }
        extra_locals.extend(
            param_bindings
                .iter()
                .map(|(id, _name, ty)| (*id, *ty, false)),
        );
        self.build_ordinary_callee_suspend_plan_for_callable(
            block,
            return_ty,
            Some(&callable_fqn),
            &extra_locals,
        )
    }

    pub(in crate::llvm::codegen) fn build_ordinary_callee_suspend_plan_impl(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        self.build_ordinary_callee_suspend_plan_for_callable(
            body,
            declared_return_ty,
            self.function_cx.current_callable_fqn.as_deref(),
            &[],
        )
    }

    pub(in crate::llvm::codegen) fn known_fun_body_may_outward_effect_impl(
        &self,
        fqn: &str,
        declared_fun_ty: TypeId,
    ) -> bool {
        let known_fun_effects = self.known_fun_body_may_outward_effect_map();
        known_fun_effects
            .get(fqn)
            .copied()
            .unwrap_or_else(|| function_ty_declared_effectful(self.types, declared_fun_ty))
    }

    pub(in crate::llvm::codegen) fn hir_ty_declared_effectful_impl(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        hir_ty.is_some_and(|ty| function_ty_declared_effectful(self.types, ty))
    }

    pub(in crate::llvm::codegen) fn local_call_may_suspend_from_hir_ty_impl(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        self.hir_ty_declared_effectful_impl(hir_ty)
    }

    pub(in crate::llvm::codegen) fn function_value_expr_body_may_outward_effect_when_called_for_local_impl(
        &self,
        expr: &hir::Expr,
    ) -> bool {
        let context = self
            .ordinary_callee_effect_analysis_ctx(self.function_cx.current_callable_fqn.as_deref());
        SuspendCallAnalysis {
            types: self.types,
            context: &context,
        }
        .function_value_may_suspend_when_called(expr, &context.known_local_fun_effects)
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
        let resume_state =
            self.begin_callee_suspend_resume_impl(at, llvm_fun, plan, incoming_resume_token)?;
        let invalid_bb = self
            .context
            .append_basic_block(llvm_fun, "ordinary_resume_invalid_site");
        let mut resume_site_blocks: Vec<(usize, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::with_capacity(plan.resume_sites.len());
        let mut cases = Vec::with_capacity(plan.resume_sites.len());

        for (index, site) in plan.resume_sites.iter().enumerate() {
            let bb = self.context.append_basic_block(
                llvm_fun,
                &format!("ordinary_resume_site{}", site.site_tag()),
            );
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
            self.builder.position_at_end(bb);
            self.function_cx.env = base_env.clone();
            self.emit_callee_suspend_resume_site_prologue_impl(at, plan, index, resume_state)?;
            let ret_v = self.codegen_block_as_return_value(
                &plan.resume_sites[index].resume_tail,
                declared_return_cg,
            )?;
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
        let saved_callable_fqn = saved_function_cx.current_callable_fqn.clone();
        let result = (|| {
            let entry = self.context.append_basic_block(resume_fun, "entry");
            self.builder.position_at_end(entry);
            self.begin_function_explicit_frame_layout(resume_fun)?;

            self.function_cx.current_callable_fqn = saved_callable_fqn.clone();
            self.function_cx.current_fun_return_ty = Some(declared_return_cg);
            let uses_hidden_sret = self
                .hidden_sret_result_ty(at, declared_return_cg)?
                .is_some();
            self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
                Some(
                    resume_fun
                        .get_nth_param(0)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing callee resume sret param",
                            at: at.into(),
                        })?
                        .into_pointer_value(),
                )
            } else {
                None
            };
            self.bind_explicit_effect_hidden_abi_slots(
                at,
                resume_fun,
                u32::from(uses_hidden_sret),
                true,
            )?;

            self.function_cx.env.push_scope();
            let (return_bb, return_alloca) =
                self.setup_function_return_context(at, resume_fun, declared_return_cg)?;
            let incoming_resume_token = self.function_cx.current_incoming_resume_token_ref.ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "missing incoming resume token ref",
                    at: at.into(),
                },
            )?;
            let base_env = self.function_cx.env.clone();

            self.with_callee_suspend_lowering(Some(plan.clone()), Some(resume_fun), |cg| {
                cg.codegen_callee_resume_dispatch(
                    at,
                    resume_fun,
                    plan,
                    &base_env,
                    declared_return_cg,
                    incoming_resume_token,
                )
            })?;

            self.emit_function_return_block(at, declared_return_cg, return_bb, return_alloca)?;
            self.finish_function_explicit_frame_layout(at)?;
            self.clear_explicit_effect_hidden_abi_slots();
            self.function_cx.current_sret_return_ptr = None;
            self.function_cx.env.pop_scope();
            Ok(())
        })();
        self.restore_function_body_cx(saved_function_cx);
        result
    }
}
