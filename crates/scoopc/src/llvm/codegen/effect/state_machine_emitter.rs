//! State machine LLVM emitter — frame type, step function, and `handle`
//! expression entry.
//!
//! This module generates LLVM IR from a `UnifiedHandleLoweringContract`:
//! - Frame struct type (system fields + user slots)
//! - Step function with state_tag-based dispatch, per-state op emission,
//!   and terminators (Goto, Branch, Suspend, CleanupEnter, ReturnHandle,
//!   ReturnFromFunction, ArmReturnHandle, ArmResumeMatchedSite,
//!   ArmMaterializeContinuation)
//! - Handle expression entry with handler arm dispatch loop
//! - Cleanup scope (finally block) execution via CleanupEnter branching
//! - Nested handle expressions via recursive codegen delegation
//!
//! All emission decisions are driven by the state machine contract.

use std::collections::{HashMap, HashSet};

use super::*;
use crate::effect::state_machine::{
    FrameSlot, HandleBranchCondition, HandleStateOp, ResumeAfterSiteReason, SuspendSiteKind,
    UnifiedArm, UnifiedFrameField, UnifiedFrameSystemField, UnifiedHandleLoweringContract,
    UnifiedState, UnifiedStateContext, UnifiedStateTerminator, UnifiedSuspendSite,
};

/// System field indices in the frame struct.
///
/// Layout:
///   field 0: header      (ScoopGcObjectHeader)
///   field 1: state_tag   (i32)   — current state / PC
///   field 2: resume_word (i64)   — scalar resume payload / handle result word
///   field 3: resume_gc_ref (ptr addrspace(1)) — GC ref resume payload / handle result ref
///   field 4+: optional system fields (cleanup_flag, one_shot_flag, completion_tag)
///   then: user slots
///   then: raw native pointers to authoritative outer-scope mutable storage
///         for metadata-driven writeback across handle exits / resumes
///   final field: suspended continuation pointer (ptr addrspace(1))
const FRAME_OBJECT_HEADER_FIELD_COUNT: u32 = 1;
const FRAME_FIELD_STATE_TAG: u32 = 1;
const FRAME_FIELD_RESUME_WORD: u32 = 2;
const FRAME_FIELD_RESUME_GC_REF: u32 = 3;

/// Sentinel state_tag value: the handle body has finished and its result is
/// available in resume_word / resume_gc_ref.  This is the normal completion
/// path — `ReturnHandle` sets this.
const STATE_TAG_HANDLE_RETURNED: u32 = 0xFFFF_FFFE;

/// Sentinel state_tag value: an early `return` statement inside the handle
/// body wants to return from the *enclosing function*, not just the handle.
/// The handle entry reads this and propagates the return upward.
const STATE_TAG_FUNCTION_RETURNED: u32 = 0xFFFF_FFFF;

fn try_rewrite_tail_resume_arm_body(
    arm: &hir::HandleArm,
    continuation_symbol: hir::SymbolId,
) -> Option<hir::Expr> {
    // 某些 escape-continuation arms 只是把 tail `k.resume(payload)` 作为 source-level surface。
    // 这里把它们保守识别成内部 tail-resume fast path，避免继续依赖公开的 `-> resume` 语法节点。
    try_rewrite_tail_resume_expr(&arm.body, continuation_symbol)
}

fn try_rewrite_tail_resume_stmt(
    stmt: &mut hir::Stmt,
    continuation_symbol: hir::SymbolId,
) -> Option<hir::Expr> {
    let hir::StmtKind::Expr(expr) = &mut stmt.kind else {
        return None;
    };
    let rewritten = try_rewrite_tail_resume_expr(expr, continuation_symbol)?;
    stmt.ty = rewritten.ty;
    *expr = rewritten.clone();
    Some(rewritten)
}

fn try_rewrite_tail_resume_expr(
    expr: &hir::Expr,
    continuation_symbol: hir::SymbolId,
) -> Option<hir::Expr> {
    if let Some(payload) = extract_tail_resume_payload_expr(expr, continuation_symbol) {
        return Some(payload);
    }

    match &expr.kind {
        hir::ExprKind::Block(block) => {
            let mut rewritten_block = block.clone();
            let tail_stmt = rewritten_block.stmts.last_mut()?;
            let rewritten_tail = try_rewrite_tail_resume_stmt(tail_stmt, continuation_symbol)?;
            rewritten_block.ty = rewritten_tail.ty;
            Some(hir::Expr {
                span: expr.span,
                ty: rewritten_tail.ty,
                kind: hir::ExprKind::Block(rewritten_block),
            })
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let rewritten_then = try_rewrite_tail_resume_expr(then_branch, continuation_symbol)?;
            let rewritten_else = else_branch.as_ref()?;
            let rewritten_else = try_rewrite_tail_resume_expr(rewritten_else, continuation_symbol)?;
            Some(hir::Expr {
                span: expr.span,
                ty: rewritten_then.ty,
                kind: hir::ExprKind::If {
                    cond: cond.clone(),
                    then_branch: Box::new(rewritten_then),
                    else_branch: Some(Box::new(rewritten_else)),
                },
            })
        }
        hir::ExprKind::When { subject, arms } => {
            let mut rewritten_arms = arms.clone();
            for arm in &mut rewritten_arms {
                arm.body = try_rewrite_tail_resume_expr(&arm.body, continuation_symbol)?;
            }
            let result_ty = rewritten_arms
                .first()
                .map(|arm| arm.body.ty)
                .unwrap_or(expr.ty);
            Some(hir::Expr {
                span: expr.span,
                ty: result_ty,
                kind: hir::ExprKind::When {
                    subject: subject.clone(),
                    arms: rewritten_arms,
                },
            })
        }
        _ => None,
    }
}

fn extract_tail_resume_payload_expr(
    expr: &hir::Expr,
    continuation_symbol: hir::SymbolId,
) -> Option<hir::Expr> {
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        return None;
    };
    let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        return None;
    };
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind else {
        return None;
    };
    if *id != continuation_symbol || member.name != "resume" {
        return None;
    }

    match args.as_slice() {
        [hir::CallArg::Positional(payload)] => Some(payload.clone()),
        [hir::CallArg::Named { value, .. }] => Some(value.clone()),
        _ => None,
    }
}

/// Tracks the frame struct layout for a specific handle expression, mapping
/// `UnifiedFrameField` indices to LLVM struct field indices.
pub(super) struct FrameLayout<'ctx> {
    pub(super) frame_type: inkwell::types::StructType<'ctx>,
    cleanup_flag_index: Option<u32>,
    completion_tag_index: Option<u32>,
    continuation_index: u32,
    outer_scope_storage_indices: HashMap<hir::SymbolId, u32>,
    ordinary_callee_resume_token_indices: HashMap<u32, u32>,
    continuation_resume_replay_token_indices: HashMap<u32, u32>,
}

impl<'ctx> FrameLayout<'ctx> {
    pub(super) fn state_tag_index(&self) -> u32 {
        FRAME_FIELD_STATE_TAG
    }

    pub(super) fn resume_word_index(&self) -> u32 {
        FRAME_FIELD_RESUME_WORD
    }

    pub(super) fn resume_gc_ref_index(&self) -> u32 {
        FRAME_FIELD_RESUME_GC_REF
    }

    pub(super) fn continuation_index(&self) -> u32 {
        self.continuation_index
    }

    pub(super) fn cleanup_flag_index(&self) -> Option<u32> {
        self.cleanup_flag_index
    }

    pub(super) fn completion_tag_index(&self) -> Option<u32> {
        self.completion_tag_index
    }

    pub(super) fn outer_scope_storage_index(&self, slot_id: hir::SymbolId) -> Option<u32> {
        self.outer_scope_storage_indices.get(&slot_id).copied()
    }

    pub(super) fn ordinary_callee_resume_token_index(&self, site_id: u32) -> Option<u32> {
        self.ordinary_callee_resume_token_indices
            .get(&site_id)
            .copied()
    }

    pub(super) fn continuation_resume_replay_token_index(&self, site_id: u32) -> Option<u32> {
        self.continuation_resume_replay_token_indices
            .get(&site_id)
            .copied()
    }

    /// Return the LLVM struct field index for a user slot given its
    /// `UnifiedFrameSlot::field_index()`.
    ///
    /// `field_index()` is already an absolute index in the frame schema
    /// (system fields first, then user slots).  物理对象布局前面还插入了
    /// 一个 GC object header，因此 schema 索引需要整体向后平移一位。
    pub(super) fn user_slot_llvm_index(&self, unified_field_index: usize) -> u32 {
        FRAME_OBJECT_HEADER_FIELD_COUNT + unified_field_index as u32
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    // ------------------------------------------------------------------
    // Frame layout generation
    // ------------------------------------------------------------------

    /// Generate the LLVM struct type for a state machine frame.
    ///
    /// The frame object layout follows
    /// `{ ScoopGcObjectHeader, UnifiedFrameSchema, suspended_continuation }`:
    /// - Object header: written by `scoop_alloc_typed`.
    /// - System fields: state_tag (i32), resume_word (i64), resume_gc_ref (ptr),
    ///   and optionally cleanup_flag (i32), one_shot_flag (i32), completion_tag (i32).
    /// - User slots: one field per `UnifiedFrameSlot`, typed according to the
    ///   slot's `TypeId`.
    /// - Outer-scope storage pointers: one native `i8*` per seeded mutable
    ///   outer slot, so both the initial handle exit and later continuation
    ///   resumes can write back through the same frame metadata contract.
    /// - Suspended continuation: a runtime-only GC ref slot appended after the
    ///   schema so step_fn re-entry can refresh public resume payload fields
    ///   without clobbering the captured continuation.
    fn emit_effect_frame_layout(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<FrameLayout<'ctx>, LlvmEmitError> {
        let frame = contract.frame();
        let system_fields = frame.fields();

        let header_ty = self.llvm_gc_object_header_type();
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();
        let native_i8_ptr_ty = self.llvm_i8_ptr_type();

        // Build the system field types in declaration order.
        let mut field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = vec![
            header_ty.into(), // GC object header
            i32_ty.into(),    // state_tag
            i64_ty.into(),    // resume_word
            gc_ptr_ty.into(), // resume_gc_ref
        ];
        let mut cleanup_flag_index = None;
        let mut completion_tag_index = None;

        // Optional system fields from the schema.
        for field in system_fields {
            match field {
                UnifiedFrameField::System(UnifiedFrameSystemField::StateTag)
                | UnifiedFrameField::System(UnifiedFrameSystemField::ResumeWord)
                | UnifiedFrameField::System(UnifiedFrameSystemField::ResumeGcRef) => {
                    // Already added above.
                }
                UnifiedFrameField::System(UnifiedFrameSystemField::CleanupFlag) => {
                    cleanup_flag_index = Some(field_types.len() as u32);
                    field_types.push(i32_ty.into());
                }
                UnifiedFrameField::System(UnifiedFrameSystemField::OneShotFlag) => {
                    field_types.push(i32_ty.into());
                }
                UnifiedFrameField::System(UnifiedFrameSystemField::CompletionTag) => {
                    completion_tag_index = Some(field_types.len() as u32);
                    field_types.push(i32_ty.into());
                }
                UnifiedFrameField::Slot { .. } => {
                    // User slots are added below.
                }
            }
        }

        // User slots: one LLVM field per UnifiedFrameSlot, in field_index order.
        let mut user_slots: Vec<(usize, crate::ty::TypeId, String)> = frame
            .slots()
            .iter()
            .map(|slot| {
                (
                    slot.field_index(),
                    slot.slot().ty(),
                    format!("{}#{}", slot.slot().name(), slot.slot().id().as_u32()),
                )
            })
            .collect();
        user_slots.sort_by_key(|(idx, _, _)| *idx);

        for (field_index, type_id, slot_name) in &user_slots {
            let Some(cg_ty) = self.cg_ty_of(*type_id) else {
                tracing::warn!(
                    field_index,
                    slot_name = %slot_name,
                    slot_ty = %self.types.display(*type_id),
                    "effect frame slot type is not codegen-lowerable"
                );
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect frame slot type",
                    at: span.into(),
                });
            };
            let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
            field_types.push(llvm_ty);
        }

        let mut outer_scope_storage_indices = HashMap::new();
        let mut writeback_slots: Vec<(usize, hir::SymbolId)> = frame
            .slots()
            .iter()
            .filter_map(|slot| {
                let frame_slot = slot.slot();
                (frame_slot.owner_arm().is_none()
                    && frame_slot.seed_from_outer_scope()
                    && frame_slot.mutable())
                .then_some((slot.field_index(), frame_slot.id()))
            })
            .collect();
        writeback_slots.sort_by_key(|(field_index, _)| *field_index);
        for (_field_index, slot_id) in writeback_slots {
            let storage_index = field_types.len() as u32;
            field_types.push(native_i8_ptr_ty.into());
            outer_scope_storage_indices.insert(slot_id, storage_index);
        }

        let mut ordinary_callee_resume_token_indices = HashMap::new();
        let mut ordinary_callee_resume_site_ids = Vec::new();
        let mut continuation_resume_site_ids = Vec::new();
        for site in contract.suspend_sites() {
            let Some(call_expr) = self.lookup_suspend_call_expr(contract, site.id()) else {
                continue;
            };
            let call_site = self.current_call_site(call_expr.span)?;
            if self.continuation_resume_call_sites.contains(&call_site) {
                continuation_resume_site_ids.push(site.id());
            } else {
                ordinary_callee_resume_site_ids.push(site.id());
            }
        }
        ordinary_callee_resume_site_ids.sort_unstable();
        for site_id in ordinary_callee_resume_site_ids {
            let token_index = field_types.len() as u32;
            field_types.push(gc_ptr_ty.into());
            ordinary_callee_resume_token_indices.insert(site_id, token_index);
        }

        let mut continuation_resume_replay_token_indices = HashMap::new();
        continuation_resume_site_ids.sort_unstable();
        for site_id in continuation_resume_site_ids {
            let token_index = field_types.len() as u32;
            field_types.push(gc_ptr_ty.into());
            continuation_resume_replay_token_indices.insert(site_id, token_index);
        }

        // Keep the suspended continuation in a dedicated runtime-only slot
        // after the schema fields so `user_slot_llvm_index` stays aligned with
        // `UnifiedFrameSchema::field_index()`.
        let continuation_index = field_types.len() as u32;
        field_types.push(gc_ptr_ty.into());

        // Create the named struct type.
        let type_name = format!("scoop.effect.frame.{:x}", span.start ^ (span.end << 16));
        let frame_type = self.context.opaque_struct_type(&type_name);
        frame_type.set_body(&field_types, false);

        Ok(FrameLayout {
            frame_type,
            cleanup_flag_index,
            completion_tag_index,
            continuation_index,
            outer_scope_storage_indices,
            ordinary_callee_resume_token_indices,
            continuation_resume_replay_token_indices,
        })
    }

    fn get_or_create_effect_frame_type_desc_global(
        &mut self,
        span: crate::span::Span,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let trace_start_offset_bytes = self
            .target_data
            .offset_of_element(&frame_layout.frame_type, FRAME_OBJECT_HEADER_FIELD_COUNT)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect frame trace_start offset",
                at: span.into(),
            })?;
        let suffix = (span.start as u64) ^ ((span.end as u64) << 16);
        let global_name = format!("__scoop_type_desc_effect_frame__{suffix:x}");
        let canonical_name = format!("scoop.effect.frame.{suffix:x}");
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at: span,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: frame_layout.frame_type,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    // ------------------------------------------------------------------
    // Step function generation
    // ------------------------------------------------------------------

    /// Generate both runtime entry points for a handle state machine.
    ///
    /// - `step_fn` 只执行状态机本体，不负责 handler dispatch。
    /// - `dispatch_loop_fn` 负责调用 `step_fn` 并在每次返回后继续跑 handler
    ///   dispatch loop，这样 escaped continuation resume 时也会重新进入
    ///   captured handler 的统一派发路径。
    fn emit_effect_runtime_functions(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<
        (
            inkwell::values::FunctionValue<'ctx>,
            inkwell::values::FunctionValue<'ctx>,
        ),
        LlvmEmitError,
    > {
        // Runtime resume entry signature: (ptr addrspace(1), i64, ptr addrspace(1)) -> void
        let state_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();

        let param_tys: [inkwell::types::BasicMetadataTypeEnum<'ctx>; 3] =
            [state_ptr_ty.into(), i64_ty.into(), gc_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);

        let suffix = span.start ^ (span.end << 16);
        let step_fn =
            self.module
                .add_function(&format!("scoop.effect.step.{suffix:x}"), fn_ty, None);
        step_fn.set_call_conventions(0);
        let dispatch_loop_fn =
            self.module
                .add_function(&format!("scoop.effect.dispatch.{suffix:x}"), fn_ty, None);
        dispatch_loop_fn.set_call_conventions(0);

        // Save caller's codegen context.
        let saved_block = self.builder.get_insert_block();
        let saved_function_cx = self.take_function_body_cx();
        let saved_effect_cx = self.take_effect_lowering_cx();
        let enclosing_return_ty =
            saved_effect_cx.enclosing_function_return_ty(saved_function_cx.current_fun_return_ty);

        // --- Generate step function body ---
        let step_result = self.emit_step_function_body(
            span,
            contract,
            frame_layout,
            step_fn,
            dispatch_loop_fn,
            enclosing_return_ty,
        );

        // Restore caller's codegen context.
        self.restore_effect_lowering_cx(saved_effect_cx);
        self.restore_function_body_cx(saved_function_cx);
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        step_result?;

        let saved_block = self.builder.get_insert_block();
        let saved_function_cx = self.take_function_body_cx();
        let saved_effect_cx = self.take_effect_lowering_cx();
        let dispatch_result = self.emit_dispatch_loop_body(
            span,
            contract,
            frame_layout,
            step_fn,
            dispatch_loop_fn,
            enclosing_return_ty,
        );

        self.restore_effect_lowering_cx(saved_effect_cx);
        self.restore_function_body_cx(saved_function_cx);

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        dispatch_result?;

        Ok((step_fn, dispatch_loop_fn))
    }

    /// Inner body of step function generation, separated so we can use `?`
    /// freely while the caller handles save/restore.
    fn emit_step_function_body(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
        step_fn: inkwell::values::FunctionValue<'ctx>,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
        enclosing_return_ty: Option<CgTy>,
    ) -> Result<(), LlvmEmitError> {
        let entry_bb = self.context.append_basic_block(step_fn, "entry");
        self.builder.position_at_end(entry_bb);
        self.begin_function_explicit_frame_layout(step_fn)?;

        // Extract parameters.
        let state_ptr = step_fn
            .get_nth_param(0)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "step fn state param",
                at: span.into(),
            })?
            .into_pointer_value();
        let state_ptr_slot = self
            .reserve_explicit_frame_leaf_slots_for_storage_type(
                span,
                self.llvm_gc_i8_ptr_type().into(),
            )?
            .into_iter()
            .next()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "step frame ptr explicit root slot",
                at: span.into(),
            })?;
        self.builder.build_store(state_ptr_slot, state_ptr)?;
        let resume_word_param = step_fn
            .get_nth_param(1)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "step fn resume_word param",
                at: span.into(),
            })?
            .into_int_value();
        let resume_gc_ref_param = step_fn
            .get_nth_param(2)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "step fn resume_gc_ref param",
                at: span.into(),
            })?
            .into_pointer_value();

        let step_function_return_ctx = if let Some(return_ty) = enclosing_return_ty {
            let return_bb = self.context.append_basic_block(step_fn, "function_return");
            let return_alloca = match return_ty {
                CgTy::Unit | CgTy::Never => None,
                _ => {
                    let return_alloca =
                        self.create_entry_alloca(span, "step_function_return_val", return_ty)?;
                    let default = self.default_value(span, return_ty)?;
                    let _ =
                        self.store_local_value_exact(span, return_alloca, return_ty, default)?;
                    Some(return_alloca)
                }
            };
            Some(EffectFunctionReturnContext {
                return_bb,
                return_alloca,
                return_ty,
            })
        } else {
            None
        };

        self.with_effect_function_return_context(step_function_return_ctx, |cg| {
            let state_ptr =
                cg.load_effect_frame_ptr_for_use(span, state_ptr_slot, "step_entry_frame_ptr")?;
            // Store resume values into frame.
            let resume_word_gep = cg.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.resume_word_index(),
                "resume_word_ptr",
            )?;
            cg.builder.build_store(resume_word_gep, resume_word_param)?;

            let resume_gc_ref_gep = cg.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.resume_gc_ref_index(),
                "resume_gc_ref_ptr",
            )?;
            cg.store_gc_ref_field(span, resume_gc_ref_gep, resume_gc_ref_param)?;

            // Load state_tag for dispatch.
            let state_tag_gep = cg.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.state_tag_index(),
                "state_tag_ptr",
            )?;
            let state_tag = cg
                .builder
                .build_load(cg.context.i32_type(), state_tag_gep, "state_tag")?
                .into_int_value();

            // Create basic blocks for each state.
            let states = contract.states();
            let unreachable_bb = cg.context.append_basic_block(step_fn, "unreachable");

            let mut state_bb_map: HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>> =
                HashMap::new();
            for state in states {
                let label = format!("state_{}", state.id());
                let bb = cg.context.append_basic_block(step_fn, &label);
                state_bb_map.insert(state.id(), bb);
            }

            // Build the switch dispatch.
            let i32_ty = cg.context.i32_type();
            let cases: Vec<_> = states
                .iter()
                .map(|state| {
                    let tag = i32_ty.const_int(state.id() as u64, false);
                    let bb = state_bb_map[&state.id()];
                    (tag, bb)
                })
                .collect();
            cg.builder.build_switch(state_tag, unreachable_bb, &cases)?;

            // Emit unreachable block.
            cg.builder.position_at_end(unreachable_bb);
            cg.builder.build_unreachable()?;

            // Emit ops and terminators for each state block.
            for state in states {
                let bb = state_bb_map[&state.id()];
                cg.builder.position_at_end(bb);

                // Fresh env scope for this state's locals.
                cg.function_cx.env.push_scope();

                let state_ptr = cg.load_effect_frame_ptr_for_use(
                    span,
                    state_ptr_slot,
                    &format!("state_{}_frame_ptr", state.id()),
                )?;

                if matches!(state.context(), UnifiedStateContext::Cleanup { .. }) {
                    cg.write_cleanup_flag(state_ptr, frame_layout, true, "cleanup_entered")?;
                }

                // Pre-populate frame slot locals so cross-state references work.
                // Each state gets its own GEP instructions (required for LLVM
                // SSA dominance: GEPs from sibling state BBs are not usable).
                cg.populate_frame_slots_in_env(span, state_ptr, frame_layout, contract)?;

                let last_value = cg.emit_state_ops(
                    span,
                    state,
                    state_ptr,
                    frame_layout,
                    contract,
                    dispatch_loop_fn,
                )?;

                let state_ptr = cg.load_effect_frame_ptr_for_use(
                    span,
                    state_ptr_slot,
                    &format!("state_{}_terminator_frame_ptr", state.id()),
                )?;

                cg.emit_state_terminator(
                    span,
                    state,
                    state.terminator(),
                    last_value,
                    state_ptr,
                    frame_layout,
                    contract,
                    &state_bb_map,
                    step_fn,
                    dispatch_loop_fn,
                )?;

                cg.function_cx.env.pop_scope();
            }

            if let Some(effect_ctx) = step_function_return_ctx {
                cg.builder.position_at_end(effect_ctx.return_bb);
                let return_value =
                    cg.load_effect_function_return_value(span, effect_ctx, "step_function_return")?;
                cg.store_result_to_frame(span, return_value, state_ptr, frame_layout)?;
                cg.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_FUNCTION_RETURNED,
                    "state_tag_step_function_return",
                )?;
                cg.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                cg.builder.build_return(None)?;
            }

            Ok(())
        })?;
        self.finish_function_explicit_frame_layout(span)?;
        Ok(())
    }

    /// Emit the reusable handler dispatch loop around `step_fn`.
    ///
    /// 约定：
    /// - 调用方负责在进入本函数前安装当前 handle 的 runtime handler frames；
    /// - 本函数只负责“推进 step_fn + 处理 perform/arm dispatch/cleanup”，并把最终
    ///   的 active flag / perform slot / frame result 留给外层调用者消费。
    fn emit_dispatch_loop_body(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
        step_fn: inkwell::values::FunctionValue<'ctx>,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
        enclosing_return_ty: Option<CgTy>,
    ) -> Result<(), LlvmEmitError> {
        let entry_bb = self.context.append_basic_block(dispatch_loop_fn, "entry");
        self.builder.position_at_end(entry_bb);
        self.begin_function_explicit_frame_layout(dispatch_loop_fn)?;

        let frame_ptr = dispatch_loop_fn
            .get_nth_param(0)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "dispatch loop state param",
                at: span.into(),
            })?
            .into_pointer_value();
        let frame_ptr_slot = self
            .reserve_explicit_frame_leaf_slots_for_storage_type(
                span,
                self.llvm_gc_i8_ptr_type().into(),
            )?
            .into_iter()
            .next()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "dispatch frame ptr explicit root slot",
                at: span.into(),
            })?;
        self.builder.build_store(frame_ptr_slot, frame_ptr)?;
        let resume_word_param = dispatch_loop_fn
            .get_nth_param(1)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "dispatch loop resume_word param",
                at: span.into(),
            })?
            .into_int_value();
        let resume_gc_ref_param = dispatch_loop_fn
            .get_nth_param(2)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "dispatch loop resume_gc_ref param",
                at: span.into(),
            })?
            .into_pointer_value();

        let dispatch_function_return_ctx = if let Some(return_ty) = enclosing_return_ty {
            let return_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "function_return");
            let return_alloca = match return_ty {
                CgTy::Unit | CgTy::Never => None,
                _ => {
                    let return_alloca =
                        self.create_entry_alloca(span, "dispatch_function_return_val", return_ty)?;
                    let default = self.default_value(span, return_ty)?;
                    let _ =
                        self.store_local_value_exact(span, return_alloca, return_ty, default)?;
                    Some(return_alloca)
                }
            };
            Some(EffectFunctionReturnContext {
                return_bb,
                return_alloca,
                return_ty,
            })
        } else {
            None
        };

        self.with_effect_function_return_context(dispatch_function_return_ctx, |cg| {
            let i64_zero = cg.context.i64_type().const_int(0, false);
            let gc_null = cg.llvm_gc_i8_ptr_type().const_null();
            let frame_ptr =
                cg.load_effect_frame_ptr_for_use(span, frame_ptr_slot, "dispatch_step_frame_ptr")?;

            cg.builder.build_call(
                step_fn,
                &[
                    frame_ptr.into(),
                    resume_word_param.into(),
                    resume_gc_ref_param.into(),
                ],
                "",
            )?;

            let dispatch_check_bb = cg
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_check");
            let dispatch_active_check_bb = cg
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_active_check");
            let dispatch_arm_bb = cg
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_arm");
            let cleanup_entry_state = contract
                .cleanup_scopes()
                .first()
                .map(|scope| scope.entry_state());
            let handle_cleanup_propagate_check_bb = cleanup_entry_state.map(|_| {
                cg.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_propagate_check")
            });
            let handle_cleanup_propagate_run_bb = cleanup_entry_state.map(|_| {
                cg.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_propagate_run")
            });
            let handle_cleanup_done_check_bb = cleanup_entry_state.map(|_| {
                cg.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_check")
            });
            let handle_cleanup_done_run_bb = cleanup_entry_state.map(|_| {
                cg.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_run")
            });
            let handle_cleanup_done_complete_bb = cleanup_entry_state.map(|_| {
                cg.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_complete")
            });
            let handle_propagate_bb = cg
                .context
                .append_basic_block(dispatch_loop_fn, "handle_propagate");
            let handle_done_bb = cg
                .context
                .append_basic_block(dispatch_loop_fn, "handle_done");
            let outward_target_bb =
                handle_cleanup_propagate_check_bb.unwrap_or(handle_propagate_bb);
            let arm_done_target_bb = handle_cleanup_done_check_bb.unwrap_or(handle_done_bb);

            cg.builder.build_unconditional_branch(dispatch_check_bb)?;

            cg.builder.position_at_end(dispatch_check_bb);
            // Terminal completion wins over any stale TLS active bit: once the
            // frame has reached a final state, the dispatch loop must continue
            // to the done/cleanup path instead of misclassifying it as an
            // outward-propagating perform.
            let dispatch_state_tag =
                cg.read_state_tag(frame_ptr, frame_layout, "dispatch_state_tag")?;
            let dispatch_terminal = cg.state_tag_matches_any(
                dispatch_state_tag,
                &[STATE_TAG_HANDLE_RETURNED, STATE_TAG_FUNCTION_RETURNED],
                "dispatch_terminal_state",
            )?;
            cg.builder.build_conditional_branch(
                dispatch_terminal,
                arm_done_target_bb,
                dispatch_active_check_bb,
            )?;

            cg.builder.position_at_end(dispatch_active_check_bb);
            let is_active = cg.emit_effect_is_active_i1(span, "handle_dispatch_is_active")?;
            cg.builder
                .build_conditional_branch(is_active, dispatch_arm_bb, arm_done_target_bb)?;

            cg.builder.position_at_end(dispatch_arm_bb);
            let performed_signal = cg.read_current_effect_signal(span, "handle_dispatch_signal")?;
            let op_tag_raw = performed_signal.op_tag;
            let effect_instance_key_raw = performed_signal.effect_instance_key;

            if contract.dispatch_entries().is_empty() {
                cg.builder.build_unconditional_branch(outward_target_bb)?;
            } else {
                let unmatched_bb = cg
                    .context
                    .append_basic_block(dispatch_loop_fn, "dispatch_unmatched");
                let arm_by_id = contract
                    .arms()
                    .iter()
                    .map(|arm| (arm.arm_id(), arm))
                    .collect::<HashMap<_, _>>();

                let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                    Vec::new();

                for dispatch_entry in contract.dispatch_entries() {
                    let op_fqn = dispatch_entry.op_fqn();
                    let tag = cg.effect_op_tag(op_fqn);
                    let tag_val = cg.context.i32_type().const_int(tag as u64, false);
                    let dispatch_arms = dispatch_entry.arms();
                    if dispatch_arms.is_empty() {
                        continue;
                    }

                    let check_blocks = dispatch_arms
                        .iter()
                        .map(|dispatch_arm| {
                            cg.context.append_basic_block(
                                dispatch_loop_fn,
                                &format!("dispatch_arm_{}_check", dispatch_arm.arm_id()),
                            )
                        })
                        .collect::<Vec<_>>();
                    cases.push((tag_val, check_blocks[0]));

                    for (index, dispatch_arm) in dispatch_arms.iter().enumerate() {
                        let unified_arm = arm_by_id.get(&dispatch_arm.arm_id()).copied().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "dispatch arm metadata not found",
                                at: span.into(),
                            },
                        )?;
                        let next_bb = check_blocks.get(index + 1).copied().unwrap_or(unmatched_bb);
                        let matching_keys = cg.matching_effect_instance_keys_for_handled_effect(
                            unified_arm.effect_ty(),
                            op_fqn,
                        );
                        let arm_bb = cg.emit_dispatch_arm_execution(
                            dispatch_loop_fn,
                            frame_ptr_slot,
                            frame_layout,
                            step_fn,
                            i64_zero,
                            gc_null,
                            unified_arm,
                            outward_target_bb,
                            arm_done_target_bb,
                            dispatch_check_bb,
                            span,
                        )?;

                        cg.builder.position_at_end(check_blocks[index]);
                        if matching_keys.is_empty() {
                            cg.builder.build_unconditional_branch(next_bb)?;
                        } else {
                            let arm_matches = cg.int_matches_any_u32(
                                effect_instance_key_raw,
                                &matching_keys,
                                &format!("arm_{}_effect_instance_match", dispatch_arm.arm_id()),
                            )?;
                            cg.builder
                                .build_conditional_branch(arm_matches, arm_bb, next_bb)?;
                        }
                    }
                }

                cg.builder.position_at_end(dispatch_arm_bb);
                if dispatch_arm_bb.get_terminator().is_none() {
                    if cases.is_empty() {
                        cg.builder.build_unconditional_branch(unmatched_bb)?;
                    } else {
                        cg.builder.build_switch(op_tag_raw, unmatched_bb, &cases)?;
                    }
                }

                cg.builder.position_at_end(unmatched_bb);
                cg.builder.build_unconditional_branch(outward_target_bb)?;
            }

            if let (
                Some(cleanup_entry_state),
                Some(cleanup_propagate_check_bb),
                Some(cleanup_propagate_run_bb),
                Some(cleanup_done_check_bb),
                Some(cleanup_done_run_bb),
                Some(cleanup_done_complete_bb),
            ) = (
                cleanup_entry_state,
                handle_cleanup_propagate_check_bb,
                handle_cleanup_propagate_run_bb,
                handle_cleanup_done_check_bb,
                handle_cleanup_done_run_bb,
                handle_cleanup_done_complete_bb,
            ) {
                cg.builder.position_at_end(cleanup_propagate_check_bb);
                let cleanup_already_ran = cg.read_cleanup_flag_i1(
                    frame_ptr,
                    frame_layout,
                    "cleanup_propagate_already_ran",
                )?;
                cg.builder.build_conditional_branch(
                    cleanup_already_ran,
                    handle_propagate_bb,
                    cleanup_propagate_run_bb,
                )?;

                cg.builder.position_at_end(cleanup_propagate_run_bb);
                let cleanup_propagate_pre_state_tag =
                    cg.read_state_tag(frame_ptr, frame_layout, "cleanup_propagate_pre_state_tag")?;
                cg.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    cleanup_entry_state,
                    "set_cleanup_propagate_state",
                )?;
                let (cleanup_resume_word, cleanup_resume_gc_ref) = cg.read_frame_resume_payload(
                    frame_ptr,
                    frame_layout,
                    "cleanup_propagate_resume_word",
                    "cleanup_propagate_resume_gc_ref",
                )?;
                let cleanup_frame_ptr = cg.load_effect_frame_ptr_for_use(
                    span,
                    frame_ptr_slot,
                    "cleanup_propagate_step_frame_ptr",
                )?;
                cg.builder.build_call(
                    step_fn,
                    &[
                        cleanup_frame_ptr.into(),
                        cleanup_resume_word.into(),
                        cleanup_resume_gc_ref.into(),
                    ],
                    "",
                )?;
                cg.restore_propagating_state_tag_after_cleanup(
                    frame_ptr,
                    frame_layout,
                    cleanup_propagate_pre_state_tag,
                    "cleanup_propagate_restore_propagating_state",
                )?;
                cg.builder.build_unconditional_branch(handle_propagate_bb)?;

                cg.builder.position_at_end(cleanup_done_check_bb);
                let cleanup_already_ran =
                    cg.read_cleanup_flag_i1(frame_ptr, frame_layout, "cleanup_done_already_ran")?;
                cg.builder.build_conditional_branch(
                    cleanup_already_ran,
                    cleanup_done_complete_bb,
                    cleanup_done_run_bb,
                )?;

                cg.builder.position_at_end(cleanup_done_run_bb);
                cg.capture_terminal_state_tag_for_cleanup(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_pre_state_tag",
                    "cleanup_done_completion_tag",
                )?;
                cg.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    cleanup_entry_state,
                    "set_cleanup_done_state",
                )?;
                let (cleanup_resume_word, cleanup_resume_gc_ref) = cg.read_frame_resume_payload(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_resume_word",
                    "cleanup_done_resume_gc_ref",
                )?;
                let cleanup_frame_ptr = cg.load_effect_frame_ptr_for_use(
                    span,
                    frame_ptr_slot,
                    "cleanup_done_step_frame_ptr",
                )?;
                cg.builder.build_call(
                    step_fn,
                    &[
                        cleanup_frame_ptr.into(),
                        cleanup_resume_word.into(),
                        cleanup_resume_gc_ref.into(),
                    ],
                    "",
                )?;
                let cleanup_active = cg.emit_effect_is_active_i1(span, "cleanup_done_is_active")?;
                cg.builder.build_conditional_branch(
                    cleanup_active,
                    handle_propagate_bb,
                    cleanup_done_check_bb,
                )?;

                cg.builder.position_at_end(cleanup_done_complete_bb);
                cg.restore_terminal_state_tag_after_cleanup(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_restore_terminal_state",
                )?;
                cg.builder.build_unconditional_branch(handle_done_bb)?;
            }

            cg.builder.position_at_end(handle_propagate_bb);
            cg.builder.build_return(None)?;

            cg.builder.position_at_end(handle_done_bb);
            let clear_fn = cg.declare_runtime_effect_clear();
            cg.builder.build_call(clear_fn, &[], "")?;
            cg.builder.build_return(None)?;

            if let Some(effect_ctx) = dispatch_function_return_ctx {
                cg.builder.position_at_end(effect_ctx.return_bb);
                let return_value = cg.load_effect_function_return_value(
                    span,
                    effect_ctx,
                    "dispatch_function_return",
                )?;
                cg.store_result_to_frame(span, return_value, frame_ptr, frame_layout)?;
                cg.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    STATE_TAG_FUNCTION_RETURNED,
                    "state_tag_dispatch_function_return",
                )?;
                cg.builder.build_unconditional_branch(arm_done_target_bb)?;
            }

            Ok(())
        })?;
        self.finish_function_explicit_frame_layout(span)?;
        Ok(())
    }

    fn rematerialize_effect_frame_ptr(
        &mut self,
        frame_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.rematerialize_ptr_in_current_block(crate::span::Span::new(0, 0), frame_ptr, name)
    }

    fn store_gc_ref_field(
        &mut self,
        at: crate::span::Span,
        slot_ptr: PointerValue<'ctx>,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_pointer_slot_with_write_barrier(at, slot_ptr, value)?;
        Ok(())
    }

    fn discard_effect_frame_continuation(
        &mut self,
        at: crate::span::Span,
        frame_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let cont_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            frame_ptr,
            frame_layout.continuation_index(),
            &format!("{label}_continuation_ptr"),
        )?;
        let continuation = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                cont_gep,
                &format!("{label}_continuation"),
            )?
            .into_pointer_value();
        let discard = self.declare_runtime_continuation_discard();
        self.builder
            .build_call(discard, &[continuation.into()], &format!("{label}_discard"))?;
        self.store_gc_ref_field(at, cont_gep, self.llvm_gc_i8_ptr_type().const_null())?;
        Ok(())
    }

    fn load_effect_frame_ptr_for_use(
        &mut self,
        _at: crate::span::Span,
        frame_ptr_slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), frame_ptr_slot, name)?
            .into_pointer_value())
    }

    /// Pre-populate the env with stable execution-time local homes for all
    /// frame user slots.
    ///
    /// Contract (T5001f8b):
    /// - The heap frame remains the persistent store across step_fn calls.
    /// - Each state BB materializes a stable entry-block alloca as the env
    ///   local home, and refreshes it from the heap frame slot on entry.
    /// - Stores performed through the env local home must write through to the
    ///   backing heap frame slot to preserve persistence.
    fn populate_frame_slots_in_env(
        &mut self,
        _span: crate::span::Span,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        for unified_slot in contract.frame().slots() {
            let slot = unified_slot.slot();
            let id = slot.id();
            let type_id = slot.ty();
            let Some(cg_ty) = self.cg_ty_of(type_id) else {
                continue; // Skip unsupported types.
            };

            let home = if let Some(existing) =
                self.function_cx.state_machine_frame_slot_homes.get(&id).copied()
            {
                existing
            } else {
                let home = self.create_entry_alloca(
                    _span,
                    &format!("handle_frame_home_{}_{}", slot.name(), id.as_u32()),
                    cg_ty,
                )?;
                self.function_cx.state_machine_frame_slot_homes.insert(id, home);
                home
            };

            let llvm_index = frame_layout.user_slot_llvm_index(unified_slot.field_index());
            let slot_ptr = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                llvm_index,
                &format!("pre_slot_{}", id.as_u32()),
            )?;

            // Refresh execution local home from the persistent heap frame.
            if cg_ty != CgTy::Never {
                let llvm_ty = self.llvm_basic_type_of(_span, cg_ty)?;
                let loaded = self.builder.build_load(
                    llvm_ty,
                    slot_ptr,
                    &format!("handle_frame_slot_load_{}", slot.name()),
                )?;
                let value = self.cg_value_from_loaded(_span, cg_ty, loaded)?;
                let _ = self.store_local_value_exact(_span, home, cg_ty, value)?;
            }

            self.function_cx.env.insert(
                id,
                CgLocal {
                    hir_ty: Some(type_id),
                    call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                    ty: cg_ty,
                    ptr: home,
                    frame_backing_ptr: Some(slot_ptr),
                    mutable: slot.mutable(),
                },
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Per-state op emission
    // ------------------------------------------------------------------

    /// Emit LLVM IR for all ops in a single state, returning the last value
    /// produced (if any).  The last value is consumed by the terminator —
    /// e.g. `ReturnHandle` stores it as the handle expression result.
    fn emit_state_ops(
        &mut self,
        span: crate::span::Span,
        state: &UnifiedState,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let mut last_value: Option<CgValue<'ctx>> = None;

        for op in state.ops() {
            let state_ptr = self.rematerialize_effect_frame_ptr(
                state_ptr,
                &format!("state_{}_op_frame_ptr", state.id()),
            )?;
            match op {
                // --- No-ops / markers ---
                HandleStateOp::StmtEmpty { .. } => {}
                HandleStateOp::WhileCondHeader { .. } => {
                    // Condition evaluation is done by the Branch terminator.
                }
                HandleStateOp::LoopReentry { .. } => {
                    // Goto to loop header is handled by the terminator.
                }
                HandleStateOp::CleanupEdgeComplete | HandleStateOp::ReturnToEnclosingExpression => {
                    // Semantic markers — no LLVM IR needed.
                    // CleanupEdgeComplete: marks end of cleanup block; the
                    //   Goto terminator on the same state handles the branch.
                    // ReturnToEnclosingExpression: marks the final state before
                    //   ReturnHandle; the terminator handles the return.
                }

                // --- Frame slot write (BindLocal) ---
                HandleStateOp::BindLocal {
                    id,
                    decl,
                    init_from_last_value,
                } => {
                    self.emit_bind_local_to_frame(
                        *id,
                        decl,
                        (*init_from_last_value).then_some(last_value).flatten(),
                        state_ptr,
                        frame_layout,
                        contract,
                    )?;
                    last_value = None;
                }

                // --- Frame slot read (ReadLocal) ---
                HandleStateOp::ReadLocal { id, expr } => {
                    let val = self.emit_read_local_from_frame(
                        *id,
                        expr.span,
                        state_ptr,
                        frame_layout,
                        contract,
                    )?;
                    last_value = Some(val);
                }

                // --- Anonymous val: evaluate for side effects, track value ---
                HandleStateOp::DeclareAnonymousVal {
                    decl,
                    init_from_last_value,
                } => {
                    if let Some(init) = &decl.init {
                        let val = if *init_from_last_value {
                            if let Some(last_value) = last_value {
                                last_value
                            } else {
                                self.codegen_expr_in_expected_context(init, None)?
                            }
                        } else {
                            self.codegen_expr_in_expected_context(init, None)?
                        };
                        last_value = Some(val);
                    } else {
                        last_value = None;
                    }
                }

                // --- Expression ops: delegate to existing codegen ---
                //
                // Standalone value references that survive into the unified
                // state machine must remain independently executable. If a
                // VarRef cannot be lowered here, that is a real production
                // bug or unsupported language feature and should surface the
                // same way it would in ordinary expr codegen.
                HandleStateOp::VarRef { expr } => {
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
                    last_value = Some(val);
                }
                HandleStateOp::Literal { expr }
                | HandleStateOp::StructLit { expr }
                | HandleStateOp::TupleLit { expr }
                | HandleStateOp::InterpolatedString { expr }
                | HandleStateOp::Expr { expr }
                | HandleStateOp::BinaryExpr { expr }
                | HandleStateOp::Call { expr }
                | HandleStateOp::WhenExpr { expr }
                | HandleStateOp::Closure { expr } => {
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
                    last_value = Some(val);
                }

                // --- Implicit else unit (if without else) ---
                HandleStateOp::ImplicitElseUnit { .. } => {
                    last_value = Some(CgValue::unit());
                }

                // --- Statement ops ---
                HandleStateOp::Assign { stmt } => {
                    self.emit_stmt_op(stmt)?;
                }

                // --- Early return: evaluate value for ReturnFromFunction ---
                HandleStateOp::Return { stmt } => {
                    if let hir::StmtKind::Return { value } = &stmt.kind {
                        if let Some(val_expr) = value {
                            // Match ordinary `return` semantics: the handle-local
                            // early-return payload must already be coerced to the
                            // enclosing function's declared return type before it
                            // is written into the shared effect transport slots.
                            let expected_return_ty = self.enclosing_function_return_ty();
                            let val = self
                                .codegen_expr_in_expected_context(val_expr, expected_return_ty)?;
                            last_value = Some(val);
                        } else {
                            last_value = Some(CgValue::unit());
                        }
                    }
                }

                // --- Break/Continue: control flow handled by terminator ---
                HandleStateOp::Break { .. } | HandleStateOp::Continue { .. } => {
                    // The state machine represents break/continue as state
                    // transitions (Goto terminator).  No LLVM IR needed here.
                }

                // --- Perform: write op_tag + payload to TLS perform slot ---
                HandleStateOp::Perform { op_fqn, expr } => {
                    self.emit_perform_op(op_fqn, expr, span)?;
                    // The Suspend terminator following this op will handle
                    // saving state, allocating continuation, and returning.
                }

                // --- Suspending call: evaluate call expression normally ---
                HandleStateOp::SuspendCall { site_id, expr } => {
                    let val = if frame_layout
                        .ordinary_callee_resume_token_index(*site_id)
                        .is_some()
                    {
                        self.emit_suspend_call_with_ordinary_callee_replay(
                            *site_id,
                            expr,
                            expr.span,
                            state_ptr,
                            frame_layout,
                            contract,
                        )?
                    } else {
                        self.with_active_suspend_site_effect_outcome_capture(
                            *site_id,
                            expr.span,
                            |cg| cg.codegen_expr_in_expected_context(expr, None),
                        )?
                    };
                    last_value = Some(val);
                    // If the callee performed, the TLS active flag is set.
                    // The Suspend terminator handles the rest.
                }

                // --- Resume landing: no-op marker ---
                HandleStateOp::ResumeAfterSite {
                    site_id,
                    reason,
                    source_span,
                    resume_slot,
                    ..
                } => {
                    // On resume, the step_fn entry block has already stored
                    // resume_word/resume_gc_ref from the parameters into the
                    // frame.  Bind that value into the synthetic resume slot
                    // so the rewritten post-suspend HIR can consume it via the
                    // normal local-read path.
                    if let Some(resume_slot) = resume_slot.as_ref() {
                        let should_replay_call = if matches!(reason, ResumeAfterSiteReason::Call) {
                            contract
                                .machine()
                                .get_suspend_site(*site_id)
                                .is_some_and(|site| {
                                    matches!(
                                        site.kind(),
                                        SuspendSiteKind::CallMaySuspend { .. }
                                            | SuspendSiteKind::CallStateMachineCallee { .. }
                                    )
                                })
                        } else {
                            false
                        };
                        if should_replay_call {
                            let val = self.emit_resume_after_call_site(
                                *site_id,
                                *source_span,
                                resume_slot,
                                state_ptr,
                                frame_layout,
                                contract,
                            )?;
                            last_value = Some(val);
                        } else {
                            self.emit_resume_value_to_frame_slot(
                                *source_span,
                                resume_slot,
                                state_ptr,
                                frame_layout,
                                contract,
                            )?;
                        }
                    }
                }

                // --- Object init access boundary: evaluate expression ---
                HandleStateOp::ObjectInitAccessBoundary { site_id, expr } => {
                    let val = self.with_active_suspend_site_effect_outcome_capture(
                        *site_id,
                        expr.span,
                        |cg| cg.codegen_expr_in_expected_context(expr, None),
                    )?;
                    last_value = Some(val);
                }

                // --- Runtime raise boundary: evaluate expression ---
                HandleStateOp::RuntimeRaiseBoundary { site_id, expr } => {
                    let val = self.with_active_suspend_site_effect_outcome_capture(
                        *site_id,
                        expr.span,
                        |cg| cg.codegen_expr_in_expected_context(expr, None),
                    )?;
                    last_value = Some(val);
                }

                // --- Execute handler arm body ---
                HandleStateOp::ExecuteArmBody {
                    arm_id,
                    op_fqn,
                    arm,
                } => {
                    let val = self.emit_execute_arm_body(
                        *arm_id,
                        op_fqn,
                        arm,
                        state_ptr,
                        frame_layout,
                        contract,
                        span,
                        dispatch_loop_fn,
                    )?;
                    last_value = Some(val);
                }

                // --- Nested handle ---
                HandleStateOp::NestedHandle { expr, .. } => {
                    // Non-suspending nested handle: delegate to codegen_expr
                    // which will recursively enter codegen_handle_expr →
                    // codegen_handle_expr_via_state_machine, generating a
                    // separate sub-state-machine for the inner handle.
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
                    last_value = Some(val);
                }
                HandleStateOp::NestedHandleBoundary { expr, .. } => {
                    // Suspending nested handle boundary: the inner handle may
                    // perform effects that bubble up.  Delegate to codegen_expr
                    // to generate the inner state machine; the outer state
                    // machine's Suspend terminator (which follows this op)
                    // handles suspension if the inner handle doesn't catch.
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
                    last_value = Some(val);
                }

                // --- Error / unsupported ops ---
                HandleStateOp::ExprMissing { expr } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing expression in state machine",
                        at: expr.span.into(),
                    });
                }
                HandleStateOp::TodoStmt { stmt, .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "todo statement in state machine",
                        at: stmt.span.into(),
                    });
                }
                HandleStateOp::TodoExpr { expr, .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "todo expression in state machine",
                        at: expr.span.into(),
                    });
                }
            }
        }

        Ok(last_value)
    }

    /// Emit a `BindLocal` op: evaluate the initializer, store to the frame
    /// slot, and register the slot GEP in the env so subsequent ops can
    /// reference this local via the standard `codegen_var_ref` path.
    fn emit_bind_local_to_frame(
        &mut self,
        id: hir::SymbolId,
        decl: &hir::ValDecl,
        init_override: Option<CgValue<'ctx>>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        let target_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bind local type in state machine",
                at: decl.span.into(),
            })?;

        // Evaluate initializer.
        let init_val = match decl.init.as_ref() {
            Some(_) => {
                if let Some(init_override) = init_override {
                    init_override
                } else {
                    self.codegen_decl_initializer_expr(decl, target_ty)?
                }
            }
            None => self.default_value(decl.span, target_ty)?,
        };

        // Find the frame slot for this local.
        let field_index = contract.frame().get_slot_field_index(id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "bind local: frame slot not found",
                at: decl.span.into(),
            },
        )?;
        let unified_slot = contract
            .frame()
            .slots()
            .iter()
            .find(|s| s.slot().id() == id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bind local: slot metadata not found",
                at: decl.span.into(),
            })?;
        let llvm_index = frame_layout.user_slot_llvm_index(field_index);

        // The initializer may already have crossed a safepoint, so rematerialize
        // the current heap frame pointer before forming the slot address.
        let state_ptr = self.rematerialize_effect_frame_ptr(
            state_ptr,
            &format!("bind_local_{}_frame", id.as_u32()),
        )?;

        // GEP into frame + store.
        let slot_ptr = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            llvm_index,
            &format!("frame_bind_{}", id.as_u32()),
        )?;
        let home = if let Some(existing) =
            self.function_cx.state_machine_frame_slot_homes.get(&id).copied()
        {
            existing
        } else {
            let home = self.create_entry_alloca(
                decl.span,
                &format!(
                    "handle_frame_home_{}_{}",
                    unified_slot.slot().name(),
                    id.as_u32()
                ),
                target_ty,
            )?;
            self.function_cx.state_machine_frame_slot_homes.insert(id, home);
            home
        };

        // Store through the stable exec local home, then write through to the persistent frame.
        let _ = self.store_local_value(decl.span, home, target_ty, init_val)?;
        let _ = self.store_local_value(decl.span, slot_ptr, target_ty, init_val)?;

        self.function_cx.env.insert(
            id,
            CgLocal {
                hir_ty: Some(decl.ty),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(decl.ty)),
                ty: target_ty,
                ptr: home,
                frame_backing_ptr: Some(slot_ptr),
                mutable: decl.mutable,
            },
        );

        Ok(())
    }

    /// Emit a `ReadLocal` op: GEP into the frame slot, load the value, and
    /// register the slot pointer in the env for subsequent access.
    fn emit_read_local_from_frame(
        &mut self,
        id: hir::SymbolId,
        at: crate::span::Span,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // Find the frame slot.
        let field_index = contract.frame().get_slot_field_index(id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "read local: frame slot not found",
                at: at.into(),
            },
        )?;
        let llvm_index = frame_layout.user_slot_llvm_index(field_index);

        // Resolve the slot's type.
        let unified_slot = contract
            .frame()
            .slots()
            .iter()
            .find(|s| s.slot().id() == id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "read local: slot metadata not found",
                at: at.into(),
            })?;
        let type_id = unified_slot.slot().ty();
        let cg_ty = self
            .cg_ty_of(type_id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "read local: unsupported slot type",
                at: at.into(),
            })?;

        let state_ptr = self.rematerialize_effect_frame_ptr(
            state_ptr,
            &format!("read_local_{}_frame", id.as_u32()),
        )?;

        // GEP into frame.
        let slot_ptr = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            llvm_index,
            &format!("frame_read_{}", id.as_u32()),
        )?;

        let home = if let Some(existing) =
            self.function_cx.state_machine_frame_slot_homes.get(&id).copied()
        {
            existing
        } else {
            let home = self.create_entry_alloca(
                at,
                &format!("handle_frame_home_{}_{}", unified_slot.slot().name(), id.as_u32()),
                cg_ty,
            )?;
            self.function_cx.state_machine_frame_slot_homes.insert(id, home);
            home
        };

        // Refresh local home for this read.
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_ty, slot_ptr, &format!("read_local_{}_from_frame", id.as_u32()))?;
        let value = self.cg_value_from_loaded(at, cg_ty, loaded)?;
        let _ = self.store_local_value_exact(at, home, cg_ty, value)?;

        // Register in env so subsequent ops can reference this local via the
        // standard `codegen_var_ref` → env lookup → load path.
        self.function_cx.env.insert(
            id,
            CgLocal {
                hir_ty: Some(type_id),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                ty: cg_ty,
                ptr: home,
                frame_backing_ptr: Some(slot_ptr),
                mutable: unified_slot.slot().mutable(),
            },
        );

        // Load and return through the standard post-safepoint reload path so
        // heap frame slots also rebuild from the current relocated base.
        let reload_slot = self.local_ptr_for_use(
            at,
            CgLocal {
                hir_ty: Some(type_id),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                ty: cg_ty,
                ptr: home,
                frame_backing_ptr: Some(slot_ptr),
                mutable: unified_slot.slot().mutable(),
            },
            &format!("read_local_{}_slot", id.as_u32()),
        )?;
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        let loaded = self.builder.build_load(llvm_ty, reload_slot, "slot_val")?;
        self.cg_value_from_loaded(at, cg_ty, loaded)
    }

    fn store_value_to_frame_slot(
        &mut self,
        at: crate::span::Span,
        resume_slot: &FrameSlot,
        value: CgValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        let field_index = contract
            .frame()
            .get_slot_field_index(resume_slot.id())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "resume slot field index",
                at: at.into(),
            })?;
        let llvm_index = frame_layout.user_slot_llvm_index(field_index);
        let state_ptr = self.rematerialize_effect_frame_ptr(
            state_ptr,
            &format!("resume_slot_frame_{}", resume_slot.id().as_u32()),
        )?;
        let slot_ptr = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            llvm_index,
            &format!("resume_slot_{}", resume_slot.id().as_u32()),
        )?;
        let cg_ty = self
            .cg_ty_of(resume_slot.ty())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "resume slot type",
                at: at.into(),
            })?;
        let value = self.coerce_value(at, value, cg_ty)?;

        let home = if let Some(existing) = self
            .function_cx
            .state_machine_frame_slot_homes
            .get(&resume_slot.id())
            .copied()
        {
            existing
        } else {
            let home = self.create_entry_alloca(
                at,
                &format!(
                    "handle_frame_home_{}_{}",
                    resume_slot.name(),
                    resume_slot.id().as_u32()
                ),
                cg_ty,
            )?;
            self.function_cx
                .state_machine_frame_slot_homes
                .insert(resume_slot.id(), home);
            home
        };

        // Store through stable exec home, then write through to persistent frame.
        let _ = self.store_local_value(at, home, cg_ty, value)?;
        let _ = self.store_local_value(at, slot_ptr, cg_ty, value)?;

        self.function_cx.env.insert(
            resume_slot.id(),
            CgLocal {
                hir_ty: Some(resume_slot.ty()),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(resume_slot.ty())),
                ty: cg_ty,
                ptr: home,
                frame_backing_ptr: Some(slot_ptr),
                mutable: resume_slot.mutable(),
            },
        );
        Ok(())
    }

    fn emit_resume_value_to_frame_slot(
        &mut self,
        at: crate::span::Span,
        resume_slot: &FrameSlot,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        let cg_ty = self
            .cg_ty_of(resume_slot.ty())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "resume slot type",
                at: at.into(),
            })?;
        let resume_value = self.read_result_from_frame(at, cg_ty, state_ptr, frame_layout)?;
        self.store_value_to_frame_slot(
            at,
            resume_slot,
            resume_value,
            state_ptr,
            frame_layout,
            contract,
        )
    }

    fn lookup_suspend_call_expr<'hir>(
        &self,
        contract: &'hir UnifiedHandleLoweringContract,
        site_id: u32,
    ) -> Option<&'hir hir::Expr> {
        contract.states().iter().find_map(|state| {
            state.ops().iter().find_map(|op| match op {
                HandleStateOp::SuspendCall {
                    site_id: op_site_id,
                    expr,
                } if *op_site_id == site_id => Some(expr.as_ref()),
                _ => None,
            })
        })
    }

    fn lookup_suspend_resume_slot(
        &self,
        contract: &UnifiedHandleLoweringContract,
        site_id: u32,
    ) -> Option<FrameSlot> {
        contract.states().iter().find_map(|state| {
            state.ops().iter().find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    site_id: resume_site_id,
                    resume_slot: Some(slot),
                    ..
                } if *resume_site_id == site_id => Some(slot.clone()),
                _ => None,
            })
        })
    }

    fn emit_suspend_call_with_ordinary_callee_replay(
        &mut self,
        site_id: u32,
        call_expr: &hir::Expr,
        source_span: crate::span::Span,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let token_index = frame_layout
            .ordinary_callee_resume_token_index(site_id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "ordinary callee replay token slot",
                at: source_span.into(),
            })?;
        let resume_slot = self.lookup_suspend_resume_slot(contract, site_id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "ordinary callee replay resume slot",
                at: source_span.into(),
            },
        )?;
        let step_fn = self.current_codegen_function(source_span)?;
        let replay_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_suspend_call_replay"));
        let fresh_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_suspend_call_fresh"));
        let merge_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_suspend_call_merge"));

        let token_slot = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            token_index,
            &format!("site{site_id}_suspend_call_callee_resume_token_ptr"),
        )?;
        let replay_token = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                token_slot,
                &format!("site{site_id}_suspend_call_callee_resume_token"),
            )?
            .into_pointer_value();
        let has_replay_token = self.ptr_is_non_null(
            source_span,
            replay_token,
            &format!("site{site_id}_suspend_call_has_callee_resume_token"),
        )?;
        self.builder
            .build_conditional_branch(has_replay_token, replay_bb, fresh_bb)?;

        self.builder.position_at_end(fresh_bb);
        let fresh_result =
            self.with_active_suspend_site_effect_outcome_capture(site_id, call_expr.span, |cg| {
                cg.codegen_expr_in_expected_context(call_expr, None)
            })?;
        self.store_value_to_frame_slot(
            source_span,
            &resume_slot,
            fresh_result,
            state_ptr,
            frame_layout,
            contract,
        )?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let explicit_outcome_slot = self.suspend_site_explicit_effect_outcome(site_id);

        self.builder.position_at_end(replay_bb);
        let (resume_word, resume_gc_ref) = self.read_frame_resume_payload(
            state_ptr,
            frame_layout,
            "suspend_call_resume_word",
            "suspend_call_resume_gc_ref",
        )?;
        self.emit_resume_payload_into_callee_suspend_state(
            source_span,
            replay_token,
            resume_word,
            resume_gc_ref,
        )?;
        self.builder
            .build_store(token_slot, self.llvm_gc_i8_ptr_type().const_null())?;
        let outcome_slot = self.alloc_effect_outcome_slot(
            source_span,
            &format!("site{site_id}_suspend_call_callee_resume"),
        )?;
        let result_cg =
            self.cg_ty_of(resume_slot.ty())
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "ordinary suspend-call replay slot type",
                    at: source_span.into(),
                })?;
        let call_result = self.with_local_never_return_semantics(|cg| {
            let call_result = cg.call_callee_resume_entry_from_state(
                source_span,
                replay_token,
                result_cg,
                &format!("site{site_id}_suspend_call_callee_resume"),
            )?;
            let deferred_call_result = cg.defer_gc_sensitive_cg_value(
                source_span,
                &format!("site{site_id}_suspend_call_callee_resume_result"),
                call_result,
            )?;
            cg.consume_current_effect_outcome_into(
                source_span,
                outcome_slot,
                &format!("site{site_id}_suspend_call_callee_resume"),
            )?;
            let replay_resume_token = cg.effect_outcome_resume_token(
                source_span,
                outcome_slot,
                &format!("site{site_id}_suspend_call_callee_resume"),
            )?;
            cg.store_gc_ref_field(source_span, token_slot, replay_resume_token)?;
            cg.emit_ordinary_call_effect_propagation_check_from_outcome(
                source_span,
                outcome_slot,
                &format!("site{site_id}_suspend_call_callee_resume"),
            )?;
            cg.materialize_deferred_cg_value(
                source_span,
                &format!("site{site_id}_suspend_call_callee_resume_result_reload"),
                deferred_call_result,
            )
        })?;
        self.store_value_to_frame_slot(
            source_span,
            &resume_slot,
            call_result,
            state_ptr,
            frame_layout,
            contract,
        )?;
        if let Some(outcome_slot) = explicit_outcome_slot {
            let outcome_tag_ptr = self.builder.build_struct_gep(
                self.llvm_effect_outcome_struct_type(),
                outcome_slot,
                0,
                &format!("site{site_id}_suspend_call_outcome_tag_ptr"),
            )?;
            self.builder
                .build_store(outcome_tag_ptr, self.context.i32_type().const_zero())?;
        }
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        self.emit_read_local_from_frame(
            resume_slot.id(),
            source_span,
            state_ptr,
            frame_layout,
            contract,
        )
    }

    fn emit_resume_after_call_site(
        &mut self,
        site_id: u32,
        source_span: crate::span::Span,
        resume_slot: &FrameSlot,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let call_expr = self.lookup_suspend_call_expr(contract, site_id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "resume.after.call source expression",
                at: source_span.into(),
            },
        )?;
        let is_continuation_resume_call = self
            .continuation_resume_call_sites
            .contains(&self.current_call_site(call_expr.span)?);
        let step_fn = self.current_codegen_function(source_span)?;
        let replay_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_replay"));
        let inactive_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_inactive"));
        let merge_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_merge"));

        let replay_token_slot = if is_continuation_resume_call {
            let token_index = frame_layout
                .continuation_resume_replay_token_index(site_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume replay token slot",
                    at: source_span.into(),
                })?;
            let token_slot = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                token_index,
                &format!("site{site_id}_continuation_resume_replay_token_ptr"),
            )?;
            let replay_token = self
                .builder
                .build_load(
                    self.llvm_gc_i8_ptr_type(),
                    token_slot,
                    &format!("site{site_id}_continuation_resume_replay_token"),
                )?
                .into_pointer_value();
            let has_token = self.ptr_is_non_null(
                source_span,
                replay_token,
                &format!("site{site_id}_has_continuation_resume_replay_token"),
            )?;
            self.builder
                .build_conditional_branch(has_token, replay_bb, inactive_bb)?;
            token_slot
        } else {
            let token_index = frame_layout
                .ordinary_callee_resume_token_index(site_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "ordinary callee resume token slot",
                    at: source_span.into(),
                })?;
            let token_slot = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                token_index,
                &format!("site{site_id}_callee_resume_token_ptr"),
            )?;
            let callee_resume_token = self
                .builder
                .build_load(
                    self.llvm_gc_i8_ptr_type(),
                    token_slot,
                    &format!("site{site_id}_callee_resume_token"),
                )?
                .into_pointer_value();
            let has_callee_resume_token = self.ptr_is_non_null(
                source_span,
                callee_resume_token,
                &format!("site{site_id}_has_callee_resume_token"),
            )?;
            self.builder.build_conditional_branch(
                has_callee_resume_token,
                replay_bb,
                inactive_bb,
            )?;
            token_slot
        };

        self.builder.position_at_end(inactive_bb);
        self.emit_resume_value_to_frame_slot(
            source_span,
            resume_slot,
            state_ptr,
            frame_layout,
            contract,
        )?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(replay_bb);
        let (resume_word, resume_gc_ref) = self.read_frame_resume_payload(
            state_ptr,
            frame_layout,
            "callee_resume_word",
            "callee_resume_gc_ref",
        )?;
        let replay_token = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                replay_token_slot,
                &format!("site{site_id}_replay_token"),
            )?
            .into_pointer_value();
        if is_continuation_resume_call {
            self.builder
                .build_store(replay_token_slot, self.llvm_gc_i8_ptr_type().const_null())?;
            let call_result = self.with_local_never_return_semantics(|cg| {
                cg.with_continuation_resume_replay(
                    ContinuationResumeReplayContext {
                        token: replay_token,
                        resume_word,
                        resume_gc_ref,
                    },
                    |cg| cg.codegen_expr_in_expected_context(call_expr, None),
                )
            })?;
            self.store_value_to_frame_slot(
                source_span,
                resume_slot,
                call_result,
                state_ptr,
                frame_layout,
                contract,
            )?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(merge_bb);
            return self.emit_read_local_from_frame(
                resume_slot.id(),
                source_span,
                state_ptr,
                frame_layout,
                contract,
            );
        }

        self.emit_resume_payload_into_callee_suspend_state(
            source_span,
            replay_token,
            resume_word,
            resume_gc_ref,
        )?;
        self.builder
            .build_store(replay_token_slot, self.llvm_gc_i8_ptr_type().const_null())?;
        let outcome_slot =
            self.alloc_effect_outcome_slot(source_span, &format!("site{site_id}_callee_resume"))?;
        let result_cg =
            self.cg_ty_of(resume_slot.ty())
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "ordinary callee resume slot type",
                    at: source_span.into(),
                })?;
        let call_result = self.with_local_never_return_semantics(|cg| {
            let call_result = cg.call_callee_resume_entry_from_state(
                source_span,
                replay_token,
                result_cg,
                &format!("site{site_id}_call_callee_resume"),
            )?;
            let deferred_call_result = cg.defer_gc_sensitive_cg_value(
                source_span,
                &format!("site{site_id}_call_callee_resume_result"),
                call_result,
            )?;
            cg.consume_current_effect_outcome_into(
                source_span,
                outcome_slot,
                &format!("site{site_id}_callee_resume"),
            )?;
            let replay_resume_token = cg.effect_outcome_resume_token(
                source_span,
                outcome_slot,
                &format!("site{site_id}_callee_resume"),
            )?;
            cg.store_gc_ref_field(source_span, replay_token_slot, replay_resume_token)?;
            cg.emit_ordinary_call_effect_propagation_check_from_outcome(
                source_span,
                outcome_slot,
                &format!("site{site_id}_callee_resume"),
            )?;
            cg.materialize_deferred_cg_value(
                source_span,
                &format!("site{site_id}_call_callee_resume_result_reload"),
                deferred_call_result,
            )
        })?;
        self.store_value_to_frame_slot(
            source_span,
            resume_slot,
            call_result,
            state_ptr,
            frame_layout,
            contract,
        )?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        self.emit_read_local_from_frame(
            resume_slot.id(),
            source_span,
            state_ptr,
            frame_layout,
            contract,
        )
    }

    fn seed_outer_scope_frame_slots(
        &mut self,
        at: crate::span::Span,
        frame_root: &DeferredCgValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        for unified_slot in contract.frame().slots() {
            let slot = unified_slot.slot();
            if slot.owner_arm().is_some() || !slot.seed_from_outer_scope() {
                continue;
            }
            let local =
                self.function_cx
                    .env
                    .get(slot.id())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect frame seed outer-scope local",
                        at: at.into(),
                    })?;

            let target_cg_ty =
                self.cg_ty_of(slot.ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect frame seed slot type",
                        at: at.into(),
                    })?;
            let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
                at,
                &format!("seed_outer_frame_{}", slot.id().as_u32()),
                frame_root,
            )?;
            let field_index = frame_layout.user_slot_llvm_index(unified_slot.field_index());
            let slot_ptr = self.builder.build_struct_gep(
                frame_layout.frame_type,
                frame_ptr,
                field_index,
                &format!("seed_outer_slot_{}", slot.id().as_u32()),
            )?;

            let value = match local.ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => {
                    let local_ptr = self.local_ptr_for_use(
                        at,
                        local,
                        &format!("seed_outer_slot_ptr_{}", slot.id().as_u32()),
                    )?;
                    let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        local_ptr,
                        &format!("seed_outer_load_{}", slot.id().as_u32()),
                    )?;
                    self.cg_value_from_loaded(at, local.ty, loaded)?
                }
            };
            let value = self.coerce_value(at, value, target_cg_ty)?;
            self.store_local_value(at, slot_ptr, target_cg_ty, value)?;

            if slot.mutable()
                && let Some(storage_index) = frame_layout.outer_scope_storage_index(slot.id())
            {
                let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
                    at,
                    &format!("seed_outer_frame_storage_{}", slot.id().as_u32()),
                    frame_root,
                )?;
                let storage_ptr_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    frame_ptr,
                    storage_index,
                    &format!("seed_outer_slot_storage_{}", slot.id().as_u32()),
                )?;
                let storage_ptr = self.builder.build_pointer_cast(
                    local.ptr,
                    self.llvm_i8_ptr_type(),
                    &format!("seed_outer_slot_storage_ptr_{}", slot.id().as_u32()),
                )?;
                self.builder.build_store(storage_ptr_gep, storage_ptr)?;
            }
        }
        Ok(())
    }

    fn promote_outer_scope_mutable_locals_to_backing_slots(
        &mut self,
        at: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        for unified_slot in contract.frame().slots() {
            let slot = unified_slot.slot();
            if slot.owner_arm().is_some() || !slot.seed_from_outer_scope() || !slot.mutable() {
                continue;
            }

            let local =
                self.function_cx
                    .env
                    .get(slot.id())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect frame promote outer-scope local",
                        at: at.into(),
                    })?;

            let backing = self.create_entry_alloca(
                at,
                &format!("handle_outer_backing_{}", slot.id().as_u32()),
                local.ty,
            )?;
            let current = match local.ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => {
                    let local_ptr = self.local_ptr_for_use(
                        at,
                        local,
                        &format!("promote_outer_slot_ptr_{}", slot.id().as_u32()),
                    )?;
                    let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        local_ptr,
                        &format!("promote_outer_load_{}", slot.id().as_u32()),
                    )?;
                    self.cg_value_from_loaded(at, local.ty, loaded)?
                }
            };
            let _ = self.store_local_value_exact(at, backing, local.ty, current)?;
            self.function_cx.env.insert(
                slot.id(),
                CgLocal {
                    hir_ty: local.hir_ty,
                    call_may_suspend: local.call_may_suspend,
                    ty: local.ty,
                    ptr: backing,
                    frame_backing_ptr: None,
                    mutable: local.mutable,
                },
            );
        }
        Ok(())
    }

    /// Write back authoritative outer-scope mutable slots from the unified
    /// handle frame to their original enclosing local storage.
    ///
    /// The original storage address is recorded in the frame itself when the
    /// handle seeds outer-scope slots. This lets the same helper run both on
    /// the initial handle exit and on later continuation-driven step-function
    /// returns, without depending on the caller's current env.
    fn write_back_outer_scope_frame_slots(
        &mut self,
        at: crate::span::Span,
        frame_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        for unified_slot in contract.frame().slots() {
            let slot = unified_slot.slot();
            if slot.owner_arm().is_some() || !slot.seed_from_outer_scope() || !slot.mutable() {
                continue;
            }
            let frame_ptr = self.rematerialize_effect_frame_ptr(
                frame_ptr,
                &format!("writeback_outer_frame_{}", slot.id().as_u32()),
            )?;

            let storage_index = frame_layout.outer_scope_storage_index(slot.id()).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "outer-scope frame writeback storage index",
                    at: at.into(),
                },
            )?;

            let slot_cg_ty =
                self.cg_ty_of(slot.ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "outer-scope frame writeback slot type",
                        at: at.into(),
                    })?;
            let field_index = frame_layout.user_slot_llvm_index(unified_slot.field_index());
            let slot_ptr = self.builder.build_struct_gep(
                frame_layout.frame_type,
                frame_ptr,
                field_index,
                &format!("writeback_outer_slot_{}", slot.id().as_u32()),
            )?;
            let storage_ptr_gep = self.builder.build_struct_gep(
                frame_layout.frame_type,
                frame_ptr,
                storage_index,
                &format!("writeback_outer_slot_storage_{}", slot.id().as_u32()),
            )?;
            let storage_ptr = self
                .builder
                .build_load(
                    self.llvm_i8_ptr_type(),
                    storage_ptr_gep,
                    &format!("writeback_outer_slot_storage_ptr_{}", slot.id().as_u32()),
                )?
                .into_pointer_value();

            let value = match slot_cg_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => {
                    let llvm_ty = self.llvm_basic_type_of(at, slot_cg_ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        slot_ptr,
                        &format!("writeback_outer_load_{}", slot.id().as_u32()),
                    )?;
                    self.cg_value_from_loaded(at, slot_cg_ty, loaded)?
                }
            };
            let caller_local_storage = self.function_cx.env.get(slot.id()).and_then(|local| {
                (local.ty == slot_cg_ty
                    && local.ptr.get_type().get_address_space() == AddressSpace::default())
                .then_some(local.ptr)
            });

            if let Some(storage_ptr) = caller_local_storage {
                // When the handle is still returning inside its original caller,
                // write back through the caller's stable backing slot so the
                // caller-facing explicit-frame home slots are refreshed too.
                self.store_local_value(at, storage_ptr, slot_cg_ty, value)?;
            } else {
                let storage_ptr = self.builder.build_pointer_cast(
                    storage_ptr,
                    self.llvm_ptr_type(AddressSpace::default()),
                    &format!("writeback_outer_slot_target_{}", slot.id().as_u32()),
                )?;
                self.store_local_value(at, storage_ptr, slot_cg_ty, value)?;
                let storage_llvm_ty = self.llvm_basic_type_of(at, slot_cg_ty)?;
                if self.basic_type_contains_gc_ptrs(at, storage_llvm_ty)? {
                    self.sync_storage_slot_into_explicit_frame(
                        at,
                        storage_ptr,
                        storage_llvm_ty,
                        &format!("writeback_outer_slot_sync_{}", slot.id().as_u32()),
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Emit a statement-type op (Assign, etc.) by dispatching to existing
    /// statement codegen.  The env must already contain the referenced locals
    /// (via prior BindLocal / ReadLocal ops in the same state).
    fn emit_stmt_op(&mut self, stmt: &hir::Stmt) -> Result<(), LlvmEmitError> {
        match &stmt.kind {
            hir::StmtKind::Empty => Ok(()),
            hir::StmtKind::Val(decl) => self.codegen_val_decl(decl),
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)
            }
            hir::StmtKind::Expr(expr) => {
                let _ = self.codegen_expr_in_expected_context(expr, Some(CgTy::Unit))?;
                Ok(())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unsupported statement in state machine",
                at: stmt.span.into(),
            }),
        }
    }

    // ------------------------------------------------------------------
    // Per-state terminator emission
    // ------------------------------------------------------------------

    /// Emit the LLVM terminator for a state block.
    #[allow(clippy::too_many_arguments)]
    fn emit_state_terminator(
        &mut self,
        span: crate::span::Span,
        state: &UnifiedState,
        terminator: &UnifiedStateTerminator,
        last_value: Option<CgValue<'ctx>>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        state_bb_map: &HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>>,
        step_fn: inkwell::values::FunctionValue<'ctx>,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        // If a suspend-related op already terminated the block (returned
        // early with `build_return`), the current block already has a
        // terminator — skip.
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_some()
        {
            return Ok(());
        }

        match terminator {
            UnifiedStateTerminator::Goto { next_state } => {
                if self.should_relay_last_value_through_goto(state, contract, *next_state)
                    && let Some(val) = last_value
                {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                let target_bb = self.lookup_state_bb(*next_state, state_bb_map, span)?;
                self.builder.build_unconditional_branch(target_bb)?;
            }

            UnifiedStateTerminator::Branch {
                condition,
                then_state,
                else_state,
                ..
            } => {
                let cond_bool = self.emit_branch_condition(condition)?;
                let then_bb = self.lookup_state_bb(*then_state, state_bb_map, span)?;
                let else_bb = self.lookup_state_bb(*else_state, state_bb_map, span)?;
                self.builder
                    .build_conditional_branch(cond_bool, then_bb, else_bb)?;
            }

            UnifiedStateTerminator::ReturnHandle => {
                // Store the handle result to the frame's resume fields.
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                // Mark completion via state_tag sentinel.
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_HANDLE_RETURNED,
                    "state_tag_handle_done",
                )?;
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::ReturnFromFunction => {
                // Store the early-return value to the frame's resume fields.
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                // Mark early function return via state_tag sentinel.
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_FUNCTION_RETURNED,
                    "state_tag_fn_return",
                )?;
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::Suspend {
                site_id,
                resume_state,
            } => {
                let site = contract.machine().get_suspend_site(*site_id).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "suspend terminator site metadata",
                        at: span.into(),
                    },
                )?;
                let resume_bb = self.lookup_state_bb(*resume_state, state_bb_map, span)?;

                if Self::suspend_site_uses_inactive_continue_path(site.kind()) {
                    let inactive_continue_bb = self
                        .context
                        .append_basic_block(step_fn, &format!("site{site_id}_inactive"));
                    let active_suspend_bb = self
                        .context
                        .append_basic_block(step_fn, &format!("site{site_id}_active"));
                    let explicit_outcome_slot =
                        self.take_suspend_site_explicit_effect_outcome(*site_id);
                    let needs_resume_token = frame_layout
                        .ordinary_callee_resume_token_index(*site_id)
                        .is_some()
                        || frame_layout
                            .continuation_resume_replay_token_index(*site_id)
                            .is_some();
                    let materialized_outcome_slot =
                        if explicit_outcome_slot.is_none() && needs_resume_token {
                            let outcome_slot = self.alloc_effect_outcome_slot(
                                span,
                                &format!("site{site_id}_tls_effect_outcome"),
                            )?;
                            self.consume_current_effect_outcome_into(
                                span,
                                outcome_slot,
                                &format!("site{site_id}_tls_effect_outcome"),
                            )?;
                            Some(outcome_slot)
                        } else {
                            None
                        };
                    let outcome_slot = explicit_outcome_slot.or(materialized_outcome_slot);
                    let is_active = if let Some(outcome_slot) = outcome_slot {
                        self.effect_outcome_is_propagating(
                            span,
                            outcome_slot,
                            &format!("site{site_id}_effect_outcome"),
                        )?
                    } else {
                        self.emit_effect_is_active_i1(span, &format!("site{site_id}_is_active"))?
                    };

                    self.builder.build_conditional_branch(
                        is_active,
                        active_suspend_bb,
                        inactive_continue_bb,
                    )?;

                    self.builder.position_at_end(inactive_continue_bb);
                    let call_result = last_value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "suspend boundary inactive continuation result",
                        at: span.into(),
                    })?;
                    self.store_result_to_frame(span, call_result, state_ptr, frame_layout)?;
                    self.builder.build_unconditional_branch(resume_bb)?;

                    self.builder.position_at_end(active_suspend_bb);
                    if let Some(outcome_slot) = outcome_slot {
                        if let Some(token_index) =
                            frame_layout.ordinary_callee_resume_token_index(*site_id)
                        {
                            let resume_token = self.effect_outcome_resume_token(
                                span,
                                outcome_slot,
                                &format!("site{site_id}_effect_outcome"),
                            )?;
                            let token_slot = self.builder.build_struct_gep(
                                frame_layout.frame_type,
                                state_ptr,
                                token_index,
                                &format!("site{site_id}_callee_resume_token_ptr"),
                            )?;
                            self.store_gc_ref_field(span, token_slot, resume_token)?;
                        }
                        if let Some(token_index) =
                            frame_layout.continuation_resume_replay_token_index(*site_id)
                        {
                            let resume_token = self.effect_outcome_resume_token(
                                span,
                                outcome_slot,
                                &format!("site{site_id}_effect_outcome"),
                            )?;
                            let token_slot = self.builder.build_struct_gep(
                                frame_layout.frame_type,
                                state_ptr,
                                token_index,
                                &format!("site{site_id}_continuation_resume_replay_token_ptr"),
                            )?;
                            self.store_gc_ref_field(span, token_slot, resume_token)?;
                        }
                        self.publish_effect_outcome_from_slot(
                            span,
                            outcome_slot,
                            &format!("site{site_id}_effect_outcome"),
                        )?;
                    } else if needs_resume_token {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "suspend outcome slot for replay token capture",
                            at: span.into(),
                        });
                    }
                }

                // Save the resume state_tag so step_fn re-enters at the
                // right state after the handler arm resumes.
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    *resume_state,
                    "state_tag_suspend_resume",
                )?;

                // Allocate a continuation object (GC-managed) that captures
                // the frame pointer and the reusable dispatch-loop entry.
                let dispatch_loop_fn_ptr = dispatch_loop_fn.as_global_value().as_pointer_value();
                let cont_alloc = self.declare_runtime_continuation_alloc();
                let cont = self
                    .builder
                    .build_call(
                        cont_alloc,
                        &[state_ptr.into(), dispatch_loop_fn_ptr.into()],
                        "continuation",
                    )?
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation_alloc return value",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let deferred_cont = self.defer_gc_ref_pointer(
                    span,
                    &format!("site{site_id}_escape_continuation"),
                    cont,
                )?;

                // Record the body resume state on the continuation itself.
                // Handler dispatch reuses frame.state_tag for arm execution,
                // so the continuation cannot rely on the mutable frame field
                // remaining pointed at the suspended body state.
                let cont = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    &format!("site{site_id}_escape_continuation_reload_state"),
                    &deferred_cont,
                )?;
                let cont_ty = self.llvm_continuation_struct_type();
                let cont_resume_state_gep = self.builder.build_struct_gep(
                    cont_ty,
                    cont,
                    2, // resume_state_tag
                    "cont_resume_state_tag",
                )?;
                let resume_state_val = self
                    .context
                    .i32_type()
                    .const_int(*resume_state as u64, false);
                self.builder
                    .build_store(cont_resume_state_gep, resume_state_val)?;
                if let Some(token_index) = frame_layout.ordinary_callee_resume_token_index(*site_id)
                {
                    let token_slot = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        token_index,
                        &format!("site{site_id}_captured_callee_resume_token_ptr"),
                    )?;
                    let captured_callee_suspend_state = self
                        .builder
                        .build_load(
                            self.llvm_gc_i8_ptr_type(),
                            token_slot,
                            &format!("site{site_id}_captured_callee_resume_token"),
                        )?
                        .into_pointer_value();
                    let set_captured =
                        self.declare_runtime_continuation_set_captured_callee_suspend_state();
                    let cont = self.reload_deferred_gc_ref_without_clearing(
                        span,
                        &format!("site{site_id}_escape_continuation_reload_capture"),
                        &deferred_cont,
                    )?;
                    self.builder.build_call(
                        set_captured,
                        &[cont.into(), captured_callee_suspend_state.into()],
                        "cont_set_captured_callee_suspend_state",
                    )?;
                }

                // Store the continuation pointer into the dedicated runtime
                // slot so later step_fn re-entry cannot overwrite it by
                // refreshing resume_gc_ref from the call parameters.
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "frame_continuation_ptr",
                )?;
                let cont = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    &format!("site{site_id}_escape_continuation_reload_frame_store"),
                    &deferred_cont,
                )?;
                self.store_gc_ref_field(span, cont_gep, cont)?;

                // `Continuation.resume(...)` resumed body 内的 suspend 只有在当前站点
                // 会把 fresh continuation 继续暴露给更外层 future resume 时，才需要
                // 留下 outer call-boundary replay 链。escape-continuation arm 的场景
                // 由 `ArmMaterializeContinuation` terminator 精确发布；这里仅处理
                // call-like boundary 与无本地 matching arm 的 outward perform。
                if Self::suspend_site_publishes_pending_continuation_during_suspend(site) {
                    let cont = self.reload_deferred_gc_ref_without_clearing(
                        span,
                        &format!("site{site_id}_escape_continuation_reload_publish"),
                        &deferred_cont,
                    )?;
                    self.emit_publish_pending_continuation(
                        cont,
                        "publish_pending_continuation_resume_inner_continuation",
                    )?;
                }
                self.clear_deferred_cg_value_root_homes(
                    span,
                    &format!("site{site_id}_escape_continuation_drop"),
                    &deferred_cont,
                )?;

                // Direct `perform` sites only wrote the TLS payload; they
                // still need to publish the active flag and source trace here.
                // Call-boundary / nested-boundary sites already arrive with an
                // active flag set by the inner producer, and re-setting it here
                // would overwrite the original perform-site trace.
                if matches!(site.kind(), SuspendSiteKind::Perform { .. }) {
                    self.emit_effect_set_active_with_trace(
                        site.span(),
                        "effect_suspend_set_active_with_trace",
                    )?;
                }

                // Return from the step function.  The dispatch loop in the
                // handle entry will detect the active flag and dispatch.
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::CleanupEnter {
                scope_id,
                next_state,
            } => {
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                // Cleanup may already have run when an escaped continuation
                // left the original handle boundary. In that case resumed-body
                // completion must skip the cleanup entry and jump straight to
                // the cleanup exit path, preserving the terminal result already
                // stored in the frame.
                let cleanup_scope = contract.machine().get_cleanup_scope(*scope_id).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "cleanup enter scope metadata",
                        at: span.into(),
                    },
                )?;
                let cleanup_entry_bb = self.lookup_state_bb(*next_state, state_bb_map, span)?;
                let cleanup_exit_bb =
                    self.lookup_state_bb(cleanup_scope.exit_state(), state_bb_map, span)?;
                let cleanup_already_ran = self.read_cleanup_flag_i1(
                    state_ptr,
                    frame_layout,
                    "cleanup_enter_already_ran",
                )?;
                self.builder.build_conditional_branch(
                    cleanup_already_ran,
                    cleanup_exit_bb,
                    cleanup_entry_bb,
                )?;
            }

            UnifiedStateTerminator::ArmReturnHandle => {
                // The arm's result becomes the handle expression result.
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_HANDLE_RETURNED,
                    "state_tag_arm_handle_done",
                )?;
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::ArmResumeMatchedSite => {
                // Tail `k.resume(...)` fast path：arm body 已计算出 resume payload（保存在
                // last_value 中）。这里直接走共享 continuation payload+answer helper，
                // 不再由 lowering/caller 直接触碰 continuation payload 字段布局。
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "read_continuation_for_resume",
                )?;
                let cont_ptr = self
                    .builder
                    .build_load(self.llvm_gc_i8_ptr_type(), cont_gep, "continuation_ref")?
                    .into_pointer_value();

                let null_slot = self.context.ptr_type(AddressSpace::default()).const_null();
                let tail_resume_slots = ContinuationResumeResultSlots {
                    out_word_slot: null_slot,
                    out_gc_ref_slot: null_slot,
                    outcome_slot: null_slot,
                };
                if let Some(val) = last_value {
                    self.resume_continuation_with_payload(
                        span,
                        cont_ptr,
                        val,
                        tail_resume_slots,
                        "continuation_resume_tail",
                    )?;
                } else {
                    self.resume_continuation_with_encoded_payload(
                        span,
                        cont_ptr,
                        self.context.i64_type().const_zero(),
                        self.llvm_gc_i8_ptr_type().const_null(),
                        tail_resume_slots,
                        "continuation_resume_tail",
                    )?;
                }

                // After resume returns, the step_fn has finished (or
                // suspended again — the dispatch loop handles that).
                // Return void to let the dispatch loop continue.
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::ArmMaterializeContinuation => {
                // EscapeContinuation arm: the continuation has already been
                // bound as a local (in ExecuteArmBody).  The arm body calls
                // k.resume() at its discretion.  If the current handle body is
                // executing under `Continuation.resume(...)`, publishing the
                // materialized continuation here preserves the outer replay
                // chain without falsely treating non-resuming / immediate arms
                // as replay sources.
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "read_continuation_for_materialize",
                )?;
                let cont_ptr = self
                    .builder
                    .build_load(
                        self.llvm_gc_i8_ptr_type(),
                        cont_gep,
                        "materialized_continuation",
                    )?
                    .into_pointer_value();
                self.emit_publish_pending_continuation(
                    cont_ptr,
                    "publish_pending_continuation_escape_arm",
                )?;

                // The arm result becomes the handle result, and the handle
                // exits with the freshly materialized continuation now owned by
                // the caller/runtime.
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_HANDLE_RETURNED,
                    "state_tag_arm_escape_done",
                )?;
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }
        }

        Ok(())
    }

    fn should_relay_last_value_through_goto(
        &self,
        state: &UnifiedState,
        contract: &UnifiedHandleLoweringContract,
        next_state: u32,
    ) -> bool {
        // Body/arm tail values may need to survive one or more transparent
        // merge states before hitting ReturnHandle, but cleanup/finally state
        // values are never the authoritative handle result. By the time we
        // enter cleanup, the real terminal payload has already been written to
        // the frame via CleanupEnter / arm completion / function-return paths.
        if matches!(state.context(), UnifiedStateContext::Cleanup { .. }) {
            return false;
        }
        self.state_preserves_handle_result_on_entry(contract, next_state)
    }

    fn state_preserves_handle_result_on_entry(
        &self,
        contract: &UnifiedHandleLoweringContract,
        state_id: u32,
    ) -> bool {
        let mut visited = HashSet::new();
        self.state_preserves_handle_result_on_entry_inner(contract, state_id, &mut visited)
    }

    fn state_preserves_handle_result_on_entry_inner(
        &self,
        contract: &UnifiedHandleLoweringContract,
        state_id: u32,
        visited: &mut HashSet<u32>,
    ) -> bool {
        // Transparent completion relays may chain through one or more empty
        // merge states before reaching the terminal ReturnHandle /
        // ReturnFromFunction block. Carry the already-computed tail value
        // across that whole no-op chain, but never through cycles or states
        // that execute real work.
        if !visited.insert(state_id) {
            return false;
        }
        let Some(state) = contract.machine().get_state(state_id) else {
            return false;
        };
        if !Self::state_ops_preserve_carried_handle_result(state.ops()) {
            return false;
        }
        match state.terminator() {
            UnifiedStateTerminator::Goto { next_state } => {
                self.state_preserves_handle_result_on_entry_inner(contract, *next_state, visited)
            }
            UnifiedStateTerminator::CleanupEnter { .. }
            | UnifiedStateTerminator::ReturnHandle
            | UnifiedStateTerminator::ReturnFromFunction
            | UnifiedStateTerminator::ArmReturnHandle
            | UnifiedStateTerminator::ArmMaterializeContinuation => true,
            UnifiedStateTerminator::Branch { .. }
            | UnifiedStateTerminator::Suspend { .. }
            | UnifiedStateTerminator::ArmResumeMatchedSite => false,
        }
    }

    fn state_ops_preserve_carried_handle_result(ops: &[HandleStateOp]) -> bool {
        ops.iter().all(|op| {
            matches!(
                op,
                HandleStateOp::StmtEmpty { .. }
                    | HandleStateOp::CleanupEdgeComplete
                    | HandleStateOp::ReturnToEnclosingExpression
            )
        })
    }

    /// Evaluate a branch condition and return the i1 boolean result.
    fn emit_branch_condition(
        &mut self,
        condition: &HandleBranchCondition,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let expr = match condition {
            HandleBranchCondition::WhileCond { condition } => condition,
            HandleBranchCondition::IfCond { condition } => condition,
        };
        let val = self.codegen_expr_in_expected_context(expr, Some(CgTy::Bool))?;
        val.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "branch condition is not bool",
            at: expr.span.into(),
        })
    }

    /// Look up the LLVM basic block for a state ID.
    fn lookup_state_bb(
        &self,
        state_id: u32,
        state_bb_map: &HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>>,
        span: crate::span::Span,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, LlvmEmitError> {
        state_bb_map
            .get(&state_id)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "state machine: target state not found",
                at: span.into(),
            })
    }

    /// Write a constant value to the frame's state_tag field.
    fn write_state_tag(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        tag_value: u32,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.state_tag_index(),
            name,
        )?;
        let val = self.context.i32_type().const_int(tag_value as u64, false);
        self.builder.build_store(gep, val)?;
        Ok(())
    }

    /// Load the frame's current `state_tag`.
    fn read_state_tag(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.state_tag_index(),
            &format!("{name}_ptr"),
        )?;
        Ok(self
            .builder
            .build_load(self.context.i32_type(), gep, name)?
            .into_int_value())
    }

    fn read_frame_resume_payload(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        word_name: &str,
        gc_name: &str,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let state_ptr =
            self.rematerialize_effect_frame_ptr(state_ptr, &format!("{word_name}_frame"))?;
        let resume_word_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.resume_word_index(),
            &format!("{word_name}_ptr"),
        )?;
        let resume_word = self
            .builder
            .build_load(self.context.i64_type(), resume_word_gep, word_name)?
            .into_int_value();

        let resume_gc_ref_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.resume_gc_ref_index(),
            &format!("{gc_name}_ptr"),
        )?;
        let resume_gc_ref = self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), resume_gc_ref_gep, gc_name)?
            .into_pointer_value();

        Ok((resume_word, resume_gc_ref))
    }

    fn write_cleanup_flag(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        value: bool,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(cleanup_flag_index) = frame_layout.cleanup_flag_index() else {
            return Ok(());
        };
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            cleanup_flag_index,
            &format!("{name}_ptr"),
        )?;
        let raw = self.context.i32_type().const_int(u64::from(value), false);
        self.builder.build_store(gep, raw)?;
        Ok(())
    }

    fn read_cleanup_flag_i1(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let Some(cleanup_flag_index) = frame_layout.cleanup_flag_index() else {
            return Ok(self.context.bool_type().const_zero());
        };
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            cleanup_flag_index,
            &format!("{name}_ptr"),
        )?;
        let raw = self
            .builder
            .build_load(self.context.i32_type(), gep, name)?
            .into_int_value();
        Ok(self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            raw,
            self.context.i32_type().const_zero(),
            &format!("{name}_bool"),
        )?)
    }

    fn write_completion_tag(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        value: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(completion_tag_index) = frame_layout.completion_tag_index() else {
            return Ok(());
        };
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            completion_tag_index,
            &format!("{name}_ptr"),
        )?;
        self.builder.build_store(gep, value)?;
        Ok(())
    }

    fn read_completion_tag(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let Some(completion_tag_index) = frame_layout.completion_tag_index() else {
            return Ok(self.context.i32_type().const_zero());
        };
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            completion_tag_index,
            &format!("{name}_ptr"),
        )?;
        Ok(self
            .builder
            .build_load(self.context.i32_type(), gep, name)?
            .into_int_value())
    }

    fn capture_terminal_state_tag_for_cleanup(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        state_tag_name: &str,
        completion_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(_completion_tag_index) = frame_layout.completion_tag_index() else {
            return Ok(());
        };
        let state_tag = self.read_state_tag(state_ptr, frame_layout, state_tag_name)?;
        let handle_returned = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            state_tag,
            self.context
                .i32_type()
                .const_int(STATE_TAG_HANDLE_RETURNED as u64, false),
            &format!("{completion_name}_handle_returned"),
        )?;
        let function_returned = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            state_tag,
            self.context
                .i32_type()
                .const_int(STATE_TAG_FUNCTION_RETURNED as u64, false),
            &format!("{completion_name}_function_returned"),
        )?;
        let is_terminal = self.builder.build_or(
            handle_returned,
            function_returned,
            &format!("{completion_name}_is_terminal"),
        )?;
        let stored_tag = self
            .builder
            .build_select(
                is_terminal,
                state_tag,
                self.context.i32_type().const_zero(),
                &format!("{completion_name}_value"),
            )?
            .into_int_value();
        self.write_completion_tag(state_ptr, frame_layout, stored_tag, completion_name)
    }

    fn restore_terminal_state_tag_after_cleanup(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(_completion_tag_index) = frame_layout.completion_tag_index() else {
            return Ok(());
        };
        let completion_tag =
            self.read_completion_tag(state_ptr, frame_layout, &format!("{name}_completion"))?;
        let current_state_tag =
            self.read_state_tag(state_ptr, frame_layout, &format!("{name}_current_state"))?;
        let has_completion = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            completion_tag,
            self.context.i32_type().const_zero(),
            &format!("{name}_has_completion"),
        )?;
        let restored_state_tag = self
            .builder
            .build_select(
                has_completion,
                completion_tag,
                current_state_tag,
                &format!("{name}_restored_state_tag"),
            )?
            .into_int_value();
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let state_tag_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.state_tag_index(),
            &format!("{name}_state_tag_ptr"),
        )?;
        self.builder
            .build_store(state_tag_gep, restored_state_tag)?;
        self.write_completion_tag(
            state_ptr,
            frame_layout,
            self.context.i32_type().const_zero(),
            &format!("{name}_clear_completion"),
        )?;
        Ok(())
    }

    fn restore_propagating_state_tag_after_cleanup(
        &mut self,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        propagating_state_tag: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let current_state_tag =
            self.read_state_tag(state_ptr, frame_layout, &format!("{name}_current_state"))?;
        let current_terminal = self.state_tag_matches_any(
            current_state_tag,
            &[STATE_TAG_HANDLE_RETURNED, STATE_TAG_FUNCTION_RETURNED],
            &format!("{name}_terminal_state"),
        )?;
        let restored_state_tag = self
            .builder
            .build_select(
                current_terminal,
                propagating_state_tag,
                current_state_tag,
                &format!("{name}_value"),
            )?
            .into_int_value();
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, &format!("{name}_frame"))?;
        let state_tag_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.state_tag_index(),
            &format!("{name}_state_tag_ptr"),
        )?;
        self.builder
            .build_store(state_tag_gep, restored_state_tag)?;
        Ok(())
    }

    /// Read the TLS effect active flag and coerce it to an LLVM `i1`.
    fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let outcome = self.read_current_effect_outcome_status(at, name)?;
        Ok(outcome.is_propagating)
    }

    fn suspend_site_uses_inactive_continue_path(kind: &SuspendSiteKind) -> bool {
        matches!(
            kind,
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::RuntimeRaise { .. }
                | SuspendSiteKind::ObjectInitAccess { .. }
                | SuspendSiteKind::TopLevelValueInitAccess { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::NestedHandleBoundary { .. }
        )
    }

    fn suspend_site_publishes_pending_continuation_during_suspend(
        site: &UnifiedSuspendSite,
    ) -> bool {
        match site.kind() {
            SuspendSiteKind::Perform { .. } => site.matching_arms().is_empty(),
            SuspendSiteKind::RuntimeRaise { .. } => false,
            SuspendSiteKind::CallMaySuspend { .. }
            | SuspendSiteKind::CallStateMachineCallee { .. }
            | SuspendSiteKind::ObjectInitAccess { .. }
            | SuspendSiteKind::TopLevelValueInitAccess { .. }
            | SuspendSiteKind::ClassCtorInit { .. }
            | SuspendSiteKind::NestedHandleBoundary { .. } => true,
        }
    }

    fn emit_publish_pending_continuation(
        &mut self,
        continuation: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let publish_pending =
            self.declare_runtime_continuation_resume_publish_pending_continuation();
        self.builder
            .build_call(publish_pending, &[continuation.into()], name)?;
        Ok(())
    }

    /// Return `true` iff `state_tag` equals one of the provided state IDs.
    fn state_tag_matches_any(
        &mut self,
        state_tag: IntValue<'ctx>,
        state_ids: &[u32],
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i1_ty = self.context.bool_type();
        let mut matches = i1_ty.const_zero();
        for (index, state_id) in state_ids.iter().enumerate() {
            let cmp = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                state_tag,
                self.context.i32_type().const_int(*state_id as u64, false),
                &format!("{name}_{index}_eq"),
            )?;
            matches = if index == 0 {
                cmp
            } else {
                self.builder
                    .build_or(matches, cmp, &format!("{name}_{index}_or"))?
            };
        }
        Ok(matches)
    }

    /// Store a CgValue into the frame's resume_word / resume_gc_ref fields,
    /// used for passing the handle result or early-return value back to the
    /// caller.
    fn store_result_to_frame(
        &mut self,
        span: crate::span::Span,
        val: CgValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let (word, gc_ref) = self.encode_effect_transport_value(span, val)?;
        let state_ptr = self.rematerialize_effect_frame_ptr(state_ptr, "store_result_frame")?;
        let word_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.resume_word_index(),
            "result_word",
        )?;
        self.builder.build_store(word_gep, word)?;
        let gc_ref_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            frame_layout.resume_gc_ref_index(),
            "result_gc_ref",
        )?;
        self.store_gc_ref_field(span, gc_ref_gep, gc_ref)?;
        Ok(())
    }

    /// Read the handle result from the frame's resume_word / resume_gc_ref
    /// fields after the step function has returned.
    fn read_result_from_frame(
        &mut self,
        span: crate::span::Span,
        result_cg_ty: CgTy,
        frame_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (word, gc_ref) = self.read_frame_resume_payload(
            frame_ptr,
            frame_layout,
            "read_result_word",
            "read_result_gc_ref",
        )?;
        self.decode_effect_transport_value(span, word, gc_ref, result_cg_ty)
    }

    fn enclosing_function_return_ty(&self) -> Option<CgTy> {
        self.effect_function_return_context()
            .map(|ctx| ctx.return_ty)
            .or(self.function_cx.current_fun_return_ty)
    }

    fn finish_enclosing_function_return_path(
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

        if let Some(effect_ctx) = self.effect_function_return_context() {
            if let Some(alloca) = effect_ctx.return_alloca
                && let Some(raw) = value.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder
                .build_unconditional_branch(effect_ctx.return_bb)?;
            return Ok(());
        }

        self.finish_function_return_path(at, declared_return_cg, value)
    }

    fn load_effect_function_return_value(
        &mut self,
        at: crate::span::Span,
        effect_ctx: EffectFunctionReturnContext<'ctx>,
        load_name: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match effect_ctx.return_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let return_alloca =
                    effect_ctx
                        .return_alloca
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect function return slot",
                            at: at.into(),
                        })?;
                let llvm_ty = self.llvm_basic_type_of(at, effect_ctx.return_ty)?;
                let loaded = self.builder.build_load(llvm_ty, return_alloca, load_name)?;
                self.cg_value_from_loaded(at, effect_ctx.return_ty, loaded)
            }
        }
    }

    // ------------------------------------------------------------------
    // Handle expression entry with dispatch loop
    // ------------------------------------------------------------------

    /// Implement `handle` expression codegen via the unified state machine.
    ///
    /// Flow:
    /// 1. Build the unified lowering contract from the `handle` HIR.
    /// 2. Generate the frame struct type and step function.
    /// 3. Allocate the frame as a GC-managed typed object.
    /// 4. Initialize the frame's state_tag to the entry state.
    /// 5. Call the step function for the initial body execution.
    /// 6. Dispatch loop: check active flag → read op_tag → dispatch to arm
    ///    → arm executes → arm may resume (re-call step_fn) or return.
    /// 7. Read the handle result from the frame.
    pub(in crate::llvm::codegen) fn codegen_handle_expr_via_state_machine(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1. Build the unified lowering contract.
        let contract = self.build_unified_lowering_contract(handle);

        // Outer mutable locals captured by a heap-backed handle frame need a stable backing slot
        // in the enclosing function; using the current explicit-root scratch slot as long-lived
        // storage lets later temporaries clobber the caller-visible value after the handle exits.
        self.promote_outer_scope_mutable_locals_to_backing_slots(span, &contract)?;

        // 2. Generate frame layout, raw state-machine step function, and the
        //    reusable dispatch-loop entry used by both initial execution and
        //    escaped-continuation resume.
        let frame_layout = self.emit_effect_frame_layout(span, &contract)?;
        let (_step_fn, dispatch_loop_fn) =
            self.emit_effect_runtime_functions(span, &contract, &frame_layout)?;

        // 3. Allocate the frame as a GC-managed typed object.
        let frame_size = self.target_data.get_store_size(&frame_layout.frame_type);
        let frame_desc = self.get_or_create_effect_frame_type_desc_global(span, &frame_layout)?;
        let frame_desc_i8 = self.builder.build_pointer_cast(
            frame_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "effect_frame_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let size_val = self.context.i64_type().const_int(frame_size, false);
        let raw_ptr = self
            .builder
            .build_call(
                rt_alloc,
                &[frame_desc_i8.into(), size_val.into()],
                "effect_frame_obj",
            )?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value (effect frame)",
                at: span.into(),
            })?
            .into_pointer_value();
        let frame_ptr = self.builder.build_pointer_cast(
            raw_ptr,
            self.llvm_ptr_type(self.gc_address_space()),
            "effect_frame_ptr",
        )?;
        let deferred_frame = self.defer_gc_ref_pointer(span, "effect_frame_obj_root", frame_ptr)?;

        // 4. Initialize the frame payload: keep the runtime-written object
        //    header intact, and clear the state-machine fields / user slots.
        let payload_offset = self
            .target_data
            .offset_of_element(&frame_layout.frame_type, frame_layout.state_tag_index())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect frame payload offset",
                at: span.into(),
            })?;
        let payload_size = frame_size.saturating_sub(payload_offset);
        if payload_size > 0 {
            let payload_gep = self.builder.build_struct_gep(
                frame_layout.frame_type,
                frame_ptr,
                frame_layout.state_tag_index(),
                "effect_frame_payload_gep",
            )?;
            let payload_i8 = self.builder.build_pointer_cast(
                payload_gep,
                self.llvm_gc_i8_ptr_type(),
                "effect_frame_payload_i8",
            )?;
            let size_ty = self.llvm_ptr_sized_int_type(None);
            let payload_size_val = size_ty.const_int(payload_size, false);
            let zero = self.context.i8_type().const_zero();
            let _ = self
                .builder
                .build_memset(payload_i8, 1, zero, payload_size_val)?;
        }

        self.seed_outer_scope_frame_slots(span, &deferred_frame, &frame_layout, &contract)?;

        // Set state_tag to entry state.
        let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "effect_frame_entry_state",
            &deferred_frame,
        )?;
        let state_tag_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            frame_ptr,
            frame_layout.state_tag_index(),
            "entry_state_tag_ptr",
        )?;
        let entry_state_val = self
            .context
            .i32_type()
            .const_int(contract.entry_state() as u64, false);
        self.builder.build_store(state_tag_gep, entry_state_val)?;

        // Runtime handler-stack registration must represent every dispatched
        // op_tag, so captured continuation contexts preserve the full dynamic
        // effect scope rather than only the first entry.
        let handler_frames = self.allocate_registered_handler_frames(&contract)?;
        let has_dispatch = !handler_frames.is_empty();

        // 5. Call the reusable dispatch-loop entry for initial body execution.
        let i64_zero = self.context.i64_type().const_int(0, false);
        let gc_null = self.llvm_gc_i8_ptr_type().const_null();
        let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "effect_frame_dispatch",
            &deferred_frame,
        )?;
        self.builder.build_call(
            dispatch_loop_fn,
            &[frame_ptr.into(), i64_zero.into(), gc_null.into()],
            "",
        )?;

        // 6. The reusable dispatch loop has finished. Inspect the effect TLS:
        //    active => outward propagation; inactive => handle completed.
        let result_cg_ty = expected
            .or_else(|| self.cg_ty_of(contract.result_ty()))
            .unwrap_or(CgTy::Unit);

        let current_fn = self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle expr: no current function",
                at: span.into(),
            })?;

        let result_slot = match result_cg_ty {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.create_entry_alloca(span, "handle_result_slot", result_cg_ty)?),
        };
        let keep_continuation = handle
            .arms
            .iter()
            .any(|arm| matches!(arm.kind, hir::HandleArmKind::EscapeContinuation { .. }));
        let handle_propagate_bb = self
            .context
            .append_basic_block(current_fn, "handle_propagate");
        let handle_done_bb = self.context.append_basic_block(current_fn, "handle_done");
        let handle_function_return_bb = self
            .context
            .append_basic_block(current_fn, "handle_function_return");
        let handle_complete_bb = self
            .context
            .append_basic_block(current_fn, "handle_complete");
        let handle_exit_bb = self.context.append_basic_block(current_fn, "handle_exit");
        let is_active = self.emit_effect_is_active_i1(span, "handle_dispatch_result_is_active")?;
        self.builder
            .build_conditional_branch(is_active, handle_propagate_bb, handle_done_bb)?;

        // --- handle_propagate: preserve active/perform slot, pop handler, and
        // let the outer boundary observe the outward-propagating effect. ---
        self.builder.position_at_end(handle_propagate_bb);

        if has_dispatch {
            self.pop_registered_handler_frames(&handler_frames)?;
        }

        let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "effect_frame_propagate",
            &deferred_frame,
        )?;
        self.write_back_outer_scope_frame_slots(span, frame_ptr, &frame_layout, &contract)?;

        if self.ordinary_effect_propagation_enabled() {
            self.clear_deferred_cg_value_root_homes(
                span,
                "effect_frame_propagate_drop",
                &deferred_frame,
            )?;
            self.emit_ordinary_non_resuming_effect_exit(span, "handle_outward_effect")?;
        }

        if let Some(result_slot) = result_slot {
            let default = self.default_value(span, result_cg_ty)?;
            self.store_local_value(span, result_slot, result_cg_ty, default)?;
        }
        self.clear_deferred_cg_value_root_homes(
            span,
            "effect_frame_propagate_drop",
            &deferred_frame,
        )?;
        self.builder.build_unconditional_branch(handle_exit_bb)?;

        // --- handle_done: the reusable dispatch loop should already have
        // cleared effect TLS, but clear again at the outer handle boundary so
        // nested-handle callers never observe a stale active bit while
        // deciding whether the call site suspended. ---
        self.builder.position_at_end(handle_done_bb);
        let clear_fn = self.declare_runtime_effect_clear();
        self.builder.build_call(clear_fn, &[], "")?;

        if has_dispatch {
            self.pop_registered_handler_frames(&handler_frames)?;
        }

        let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "effect_frame_done",
            &deferred_frame,
        )?;
        self.write_back_outer_scope_frame_slots(span, frame_ptr, &frame_layout, &contract)?;

        let post_state_tag = self.read_state_tag(frame_ptr, &frame_layout, "post_state_tag")?;
        let function_returned = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            post_state_tag,
            self.context
                .i32_type()
                .const_int(STATE_TAG_FUNCTION_RETURNED as u64, false),
            "post_state_tag_function_returned",
        )?;
        self.builder.build_conditional_branch(
            function_returned,
            handle_function_return_bb,
            handle_complete_bb,
        )?;

        self.builder.position_at_end(handle_function_return_bb);
        let declared_return_cg =
            self.enclosing_function_return_ty()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle function return type",
                    at: span.into(),
                })?;
        let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "effect_frame_function_return",
            &deferred_frame,
        )?;
        if !keep_continuation {
            self.discard_effect_frame_continuation(
                span,
                frame_ptr,
                &frame_layout,
                "effect_frame_function_return",
            )?;
        }
        let return_value =
            self.read_result_from_frame(span, declared_return_cg, frame_ptr, &frame_layout)?;
        self.clear_deferred_cg_value_root_homes(
            span,
            "effect_frame_function_return_drop",
            &deferred_frame,
        )?;
        self.finish_enclosing_function_return_path(span, declared_return_cg, return_value)?;

        self.builder.position_at_end(handle_complete_bb);

        if let Some(result_slot) = result_slot {
            let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
                span,
                "effect_frame_complete",
                &deferred_frame,
            )?;
            if !keep_continuation {
                self.discard_effect_frame_continuation(
                    span,
                    frame_ptr,
                    &frame_layout,
                    "effect_frame_complete",
                )?;
            }
            let result =
                self.read_result_from_frame(span, result_cg_ty, frame_ptr, &frame_layout)?;
            self.store_local_value(span, result_slot, result_cg_ty, result)?;
        } else {
            let frame_ptr = self.reload_deferred_gc_ref_without_clearing(
                span,
                "effect_frame_complete",
                &deferred_frame,
            )?;
            if !keep_continuation {
                self.discard_effect_frame_continuation(
                    span,
                    frame_ptr,
                    &frame_layout,
                    "effect_frame_complete",
                )?;
            }
        }
        self.clear_deferred_cg_value_root_homes(
            span,
            "effect_frame_complete_drop",
            &deferred_frame,
        )?;
        self.builder.build_unconditional_branch(handle_exit_bb)?;

        self.builder.position_at_end(handle_exit_bb);
        match result_cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let result_slot = result_slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result slot missing",
                    at: span.into(),
                })?;
                let llvm_ty = self.llvm_basic_type_of(span, result_cg_ty)?;
                let raw = self
                    .builder
                    .build_load(llvm_ty, result_slot, "handle_result")?;
                self.cg_value_from_loaded(span, result_cg_ty, raw)
            }
        }
    }

    // ------------------------------------------------------------------
    // Helper methods: perform op, arm body, resume payload
    // ------------------------------------------------------------------

    fn int_matches_any_u32(
        &mut self,
        value: IntValue<'ctx>,
        candidates: &[u32],
        label: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if candidates.is_empty() {
            return Ok(self.context.bool_type().const_zero());
        }

        let int_ty = value.get_type();
        let mut matched = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            value,
            int_ty.const_int(candidates[0] as u64, false),
            &format!("{label}_0"),
        )?;
        for (index, candidate) in candidates.iter().copied().enumerate().skip(1) {
            let cmp = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                value,
                int_ty.const_int(candidate as u64, false),
                &format!("{label}_{index}"),
            )?;
            matched = self
                .builder
                .build_or(matched, cmp, &format!("{label}_or_{index}"))?;
        }

        Ok(matched)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_dispatch_arm_execution(
        &mut self,
        current_fn: FunctionValue<'ctx>,
        frame_ptr_slot: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        step_fn: FunctionValue<'ctx>,
        i64_zero: IntValue<'ctx>,
        gc_null: PointerValue<'ctx>,
        unified_arm: &UnifiedArm,
        outward_target_bb: inkwell::basic_block::BasicBlock<'ctx>,
        arm_done_target_bb: inkwell::basic_block::BasicBlock<'ctx>,
        dispatch_check_bb: inkwell::basic_block::BasicBlock<'ctx>,
        span: crate::span::Span,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, LlvmEmitError> {
        let arm_id = unified_arm.arm_id();
        let arm_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm_{arm_id}"));
        let arm_effect_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm_{arm_id}_effect"));
        let arm_complete_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm_{arm_id}_complete"));

        self.builder.position_at_end(arm_bb);
        let frame_ptr = self.load_effect_frame_ptr_for_use(
            span,
            frame_ptr_slot,
            &format!("arm_{arm_id}_frame"),
        )?;

        let clear_active_fn = self.declare_runtime_effect_clear_active();
        self.builder.build_call(clear_active_fn, &[], "")?;

        self.write_state_tag(
            frame_ptr,
            frame_layout,
            unified_arm.entry_state(),
            &format!("set_arm_state_{arm_id}"),
        )?;
        let frame_ptr =
            self.rematerialize_effect_frame_ptr(frame_ptr, &format!("arm_{arm_id}_step_frame"))?;

        self.builder.build_call(
            step_fn,
            &[frame_ptr.into(), i64_zero.into(), gc_null.into()],
            "",
        )?;

        let arm_active = self.emit_effect_is_active_i1(span, &format!("arm_{arm_id}_is_active"))?;
        self.builder
            .build_conditional_branch(arm_active, arm_effect_bb, arm_complete_bb)?;

        self.builder.position_at_end(arm_effect_bb);
        let arm_state_tag =
            self.read_state_tag(frame_ptr, frame_layout, &format!("arm_{arm_id}_state_tag"))?;
        let arm_context_active = self.state_tag_matches_any(
            arm_state_tag,
            unified_arm.body_states(),
            &format!("arm_{arm_id}_body_state"),
        )?;
        self.builder.build_conditional_branch(
            arm_context_active,
            outward_target_bb,
            dispatch_check_bb,
        )?;

        self.builder.position_at_end(arm_complete_bb);
        let arm_complete_state_tag = self.read_state_tag(
            frame_ptr,
            frame_layout,
            &format!("arm_{arm_id}_complete_state_tag"),
        )?;
        let arm_handle_returned = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            arm_complete_state_tag,
            self.context
                .i32_type()
                .const_int(STATE_TAG_HANDLE_RETURNED as u64, false),
            &format!("arm_{arm_id}_handle_returned"),
        )?;
        let arm_function_returned = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            arm_complete_state_tag,
            self.context
                .i32_type()
                .const_int(STATE_TAG_FUNCTION_RETURNED as u64, false),
            &format!("arm_{arm_id}_function_returned"),
        )?;
        let arm_terminal = self.builder.build_or(
            arm_handle_returned,
            arm_function_returned,
            &format!("arm_{arm_id}_terminal"),
        )?;
        self.builder.build_conditional_branch(
            arm_terminal,
            arm_done_target_bb,
            dispatch_check_bb,
        )?;

        Ok(arm_bb)
    }

    /// Emit a `perform` op: evaluate the perform expression's args and write
    /// the op_tag + payload to the TLS perform slot.
    fn emit_perform_op(
        &mut self,
        op_fqn: &str,
        expr: &hir::Expr,
        span: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let op_tag = self.effect_op_tag(op_fqn);
        let op_tag_val = self.context.i32_type().const_int(op_tag as u64, false);
        let effect_ty = match &expr.kind {
            hir::ExprKind::Perform { effect_ty, .. } => *effect_ty,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "state machine perform effect type",
                    at: expr.span.into(),
                });
            }
        };
        let effect_instance_key =
            self.effect_instance_key(effect_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "state machine perform effect instance key",
                    at: expr.span.into(),
                })?;
        let effect_instance_key_val = self
            .context
            .i32_type()
            .const_int(effect_instance_key as u64, false);

        let args = match &expr.kind {
            hir::ExprKind::Perform { args, .. } => args.as_slice(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "state machine perform payload expr",
                    at: expr.span.into(),
                });
            }
        };

        // Reuse the ordinary perform transport helper so state-machine direct
        // perform sites and indirect ordinary perform share the same tuple
        // transport contract for 2+ payload args.
        let payload_val = self.codegen_perform_payload_value(expr.span, args)?;

        let (word, gc_ref) = self.encode_effect_transport_value(span, payload_val)?;
        let write_fn = self.declare_runtime_effect_perform_slot_write_u64_with_gc_ref();
        self.builder.build_call(
            write_fn,
            &[
                op_tag_val.into(),
                effect_instance_key_val.into(),
                word.into(),
                gc_ref.into(),
            ],
            "",
        )?;

        Ok(())
    }

    /// Emit the body of a handler arm: set up binder locals from the TLS
    /// perform slot, set up continuation reference if needed, then execute
    /// the arm body expression.
    #[allow(clippy::too_many_arguments)]
    fn emit_execute_arm_body(
        &mut self,
        arm_id: u32,
        _op_fqn: &str,
        arm: &hir::HandleArm,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        span: crate::span::Span,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // Find the unified arm metadata.
        let unified_arm = contract
            .arms()
            .iter()
            .find(|a| a.arm_id() == arm_id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "arm not found in contract",
                at: arm.span.into(),
            })?;

        let multi_binder_payload = if arm.op.binders.len() > 1 {
            let tuple_ty = self.handle_payload_tuple_ty_for_span(arm.op.span)?.ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-binder payload tuple",
                    at: arm.op.span.into(),
                },
            )?;
            Some((
                tuple_ty,
                self.read_perform_slot_payload(arm.op.span, CgTy::Tuple(tuple_ty))?,
            ))
        } else {
            None
        };

        // Set up binder locals: read from perform slot and store to frame slots.
        for (binder_idx, binder) in arm.op.binders.iter().enumerate() {
            let binder_cg_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "arm binder type",
                        at: binder.span.into(),
                    })?;

            let binder_val = if let Some((tuple_ty, payload)) = multi_binder_payload {
                self.extract_tuple_payload_element(
                    binder.span,
                    payload,
                    tuple_ty,
                    binder_idx as u32,
                )?
            } else {
                self.read_binder_from_perform_slot(binder.span, binder_cg_ty)?
            };

            // If there's a frame slot for this binder, store to frame.
            if let Some(field_index) = contract.frame().get_slot_field_index(binder.id) {
                let state_ptr = self.rematerialize_effect_frame_ptr(
                    state_ptr,
                    &format!("arm_binder_frame_{}", binder.id.as_u32()),
                )?;
                let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                let slot_ptr = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    llvm_index,
                    &format!("arm_binder_{}", binder.id.as_u32()),
                )?;

                let home = if let Some(existing) = self
                    .function_cx
                    .state_machine_frame_slot_homes
                    .get(&binder.id)
                    .copied()
                {
                    existing
                } else {
                    let home = self.create_entry_alloca(
                        binder.span,
                        &format!("handle_frame_home_{}_{}", binder.name, binder.id.as_u32()),
                        binder_cg_ty,
                    )?;
                    self.function_cx
                        .state_machine_frame_slot_homes
                        .insert(binder.id, home);
                    home
                };

                // Store through exec home, then write through to persistent frame.
                let _ = self.store_local_value(binder.span, home, binder_cg_ty, binder_val)?;
                let _ = self.store_local_value(binder.span, slot_ptr, binder_cg_ty, binder_val)?;
                self.function_cx.env.insert(
                    binder.id,
                    CgLocal {
                        hir_ty: Some(binder.ty),
                        call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(binder.ty)),
                        ty: binder_cg_ty,
                        ptr: home,
                        frame_backing_ptr: Some(slot_ptr),
                        mutable: false,
                    },
                );
            } else {
                // No frame slot — still spill through an entry-block alloca so
                // statepoint rewriting can treat it like an ordinary local root.
                let alloca = self.create_entry_alloca(
                    binder.span,
                    &format!("binder_{}", binder.name),
                    binder_cg_ty,
                )?;
                self.store_local_value(binder.span, alloca, binder_cg_ty, binder_val)?;
                self.function_cx.env.insert(
                    binder.id,
                    CgLocal {
                        hir_ty: Some(binder.ty),
                        call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(binder.ty)),
                        ty: binder_cg_ty,
                        ptr: alloca,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                );
            }
        }

        // EscapeContinuation arms bind the continuation as a local；若其 body 恰好是
        // tail `k.resume(...)`，后续会被内部 fast path 识别并改写。
        match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                // Load the continuation pointer from the dedicated runtime
                // continuation slot (where Suspend stored it).
                let continuation_hir_ty = self
                    .function_cx
                    .env
                    .get(continuation)
                    .and_then(|local| local.hir_ty);
                let continuation_call_may_suspend =
                    self.local_call_may_suspend_from_hir_ty(continuation_hir_ty);
                let state_ptr = self.rematerialize_effect_frame_ptr(
                    state_ptr,
                    &format!("arm_continuation_frame_{arm_id}"),
                )?;
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "load_continuation",
                )?;
                let cont_ptr = self.resolve_escape_continuation_for_arm(
                    span,
                    arm_id,
                    state_ptr,
                    cont_gep,
                    frame_layout,
                    contract,
                    dispatch_loop_fn,
                )?;
                self.retarget_escaped_continuation_resume_state(arm_id, cont_ptr, contract)?;

                // Find or alloc frame slot for the continuation local.
                if let Some(field_index) = contract.frame().get_slot_field_index(continuation) {
                    let state_ptr = self.rematerialize_effect_frame_ptr(
                        state_ptr,
                        &format!("arm_cont_slot_frame_{}", continuation.as_u32()),
                    )?;
                    let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                    let slot_ptr = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        llvm_index,
                        "cont_slot",
                    )?;

                    let home = if let Some(existing) = self
                        .function_cx
                        .state_machine_frame_slot_homes
                        .get(&continuation)
                        .copied()
                    {
                        existing
                    } else {
                        let home = self.create_entry_alloca(
                            span,
                            &format!("handle_frame_home_cont_{}", continuation.as_u32()),
                            CgTy::Ref,
                        )?;
                        self.function_cx
                            .state_machine_frame_slot_homes
                            .insert(continuation, home);
                        home
                    };

                    // Store through exec home, then write through to persistent frame.
                    let value = CgValue {
                        ty: CgTy::Ref,
                        value: Some(cont_ptr.into()),
                    };
                    let _ = self.store_local_value_exact(span, home, CgTy::Ref, value)?;
                    let _ = self.store_local_value_exact(span, slot_ptr, CgTy::Ref, value)?;
                    self.function_cx.env.insert(
                        continuation,
                        CgLocal {
                            hir_ty: continuation_hir_ty,
                            call_may_suspend: continuation_call_may_suspend,
                            ty: CgTy::Ref,
                            ptr: home,
                            frame_backing_ptr: Some(slot_ptr),
                            mutable: false,
                        },
                    );
                } else {
                    let alloca = self.create_entry_alloca(span, "cont_local", CgTy::Ref)?;
                    let _ = self.store_local_value_exact(
                        span,
                        alloca,
                        CgTy::Ref,
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(cont_ptr.into()),
                        },
                    )?;
                    self.function_cx.env.insert(
                        continuation,
                        CgLocal {
                            hir_ty: continuation_hir_ty,
                            call_may_suspend: continuation_call_may_suspend,
                            ty: CgTy::Ref,
                            ptr: alloca,
                            frame_backing_ptr: None,
                            mutable: false,
                        },
                    );
                }
            }
            hir::HandleArmKind::NonResuming => {
                // No special setup needed.
            }
        }

        // Restore captured locals from the frame into the env so the arm
        // body can reference them.
        for &local_id in unified_arm.capture_locals() {
            if self.function_cx.env.get(local_id).is_some() {
                continue; // Already in env from binder setup.
            }
            // Try to load from frame.
            if let Some(field_index) = contract.frame().get_slot_field_index(local_id)
                && let Some(slot) = contract
                    .frame()
                    .slots()
                    .iter()
                    .find(|s| s.slot().id() == local_id)
            {
                let type_id = slot.slot().ty();
                if let Some(cg_ty) = self.cg_ty_of(type_id) {
                    let state_ptr = self.rematerialize_effect_frame_ptr(
                        state_ptr,
                        &format!("arm_capture_frame_{}", local_id.as_u32()),
                    )?;
                    let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                    let slot_ptr = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        llvm_index,
                        &format!("capture_{}", local_id.as_u32()),
                    )?;

                    let home = if let Some(existing) = self
                        .function_cx
                        .state_machine_frame_slot_homes
                        .get(&local_id)
                        .copied()
                    {
                        existing
                    } else {
                        let home = self.create_entry_alloca(
                            span,
                            &format!(
                                "handle_frame_home_capture_{}_{}",
                                slot.slot().name(),
                                local_id.as_u32()
                            ),
                            cg_ty,
                        )?;
                        self.function_cx
                            .state_machine_frame_slot_homes
                            .insert(local_id, home);
                        home
                    };

                    // Refresh the exec home from the persistent heap frame slot.
                    let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, slot_ptr, &format!("capture_load_{}", local_id.as_u32()))?;
                    let value = self.cg_value_from_loaded(span, cg_ty, loaded)?;
                    let _ = self.store_local_value_exact(span, home, cg_ty, value)?;

                    self.function_cx.env.insert(
                        local_id,
                        CgLocal {
                            hir_ty: Some(type_id),
                            call_may_suspend: self
                                .local_call_may_suspend_from_hir_ty(Some(type_id)),
                            ty: cg_ty,
                            ptr: home,
                            frame_backing_ptr: Some(slot_ptr),
                            mutable: slot.slot().mutable(),
                        },
                    );
                }
            }
        }

        // Execute the arm body under the same "active => immediately exit the
        // current frame" contract used by ordinary callees.
        //
        // Arm bodies are currently emitted as one opaque expression tree
        // instead of being segmented into state-machine ops. Without a
        // temporary ordinary-frame return type, a non-resuming effect raised
        // inside the arm would only set TLS active and then keep executing the
        // rest of the arm body. `with_local_never_return_semantics(...)`
        // lets the existing ordinary propagation helpers terminate the step
        // function immediately when arm-local code performs.

        let tail_resume_rewritten = match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                try_rewrite_tail_resume_arm_body(arm, continuation)
            }
            hir::HandleArmKind::NonResuming => None,
        };

        self.with_local_never_return_semantics(|cg| match tail_resume_rewritten {
            Some(rewritten) => {
                let payload_cg_ty =
                    cg.cg_ty_of(rewritten.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "tail resume payload type",
                            at: rewritten.span.into(),
                        })?;
                cg.codegen_expr_in_expected_context(&rewritten, Some(payload_cg_ty))
            }
            None => cg.codegen_expr_in_expected_context(&arm.body, None),
        })
    }

    fn retarget_escaped_continuation_resume_state(
        &mut self,
        arm_id: u32,
        cont_ptr: PointerValue<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        let replay_sites = contract
            .suspend_sites()
            .iter()
            .filter_map(|site| {
                site.escape_resume_state().map(|escape_resume_state| {
                    (site.resume_state(), escape_resume_state, site.id())
                })
            })
            .collect::<Vec<_>>();
        if replay_sites.is_empty() {
            return Ok(());
        }

        let cont_ty = self.llvm_continuation_struct_type();
        let resume_state_gep = self.builder.build_struct_gep(
            cont_ty,
            cont_ptr,
            2, // resume_state_tag
            "cont_escape_resume_state_tag",
        )?;
        let current_tag = self
            .builder
            .build_load(
                self.context.i32_type(),
                resume_state_gep,
                "cont_escape_resume_state_load",
            )?
            .into_int_value();

        let mut replay_tag = current_tag;
        for (index, (resume_state, escape_resume_state, site_id)) in replay_sites.iter().enumerate()
        {
            let matches_site = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_tag,
                self.context
                    .i32_type()
                    .const_int(*resume_state as u64, false),
                &format!("arm{arm_id}_site{site_id}_escape_resume_match"),
            )?;
            replay_tag = self
                .builder
                .build_select(
                    matches_site,
                    self.context
                        .i32_type()
                        .const_int(*escape_resume_state as u64, false),
                    replay_tag,
                    &format!("arm{arm_id}_escape_resume_tag{index}"),
                )?
                .into_int_value();
        }

        self.builder.build_store(resume_state_gep, replay_tag)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_escape_continuation_for_arm(
        &mut self,
        span: crate::span::Span,
        arm_id: u32,
        state_ptr: PointerValue<'ctx>,
        cont_gep: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        dispatch_loop_fn: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let existing = self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), cont_gep, "continuation_val")?
            .into_pointer_value();

        let matching_sites: Vec<_> = contract.suspend_sites().iter().collect();
        if matching_sites.is_empty() {
            return Ok(existing);
        }

        let direct_site = matching_sites.iter().copied().find(|site| {
            frame_layout
                .ordinary_callee_resume_token_index(site.id())
                .is_none()
        });
        let indirect_sites: Vec<_> = matching_sites
            .iter()
            .copied()
            .filter(|site| {
                frame_layout
                    .ordinary_callee_resume_token_index(site.id())
                    .is_some()
            })
            .collect();

        let current_fn = self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "escape continuation current function",
                at: span.into(),
            })?;

        let existing_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm{arm_id}_cont_existing"));
        let mut next_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm{arm_id}_cont_from_token"));
        let merge_bb = self
            .context
            .append_basic_block(current_fn, &format!("arm{arm_id}_cont_merge"));

        let has_existing =
            self.ptr_is_non_null(span, existing, &format!("arm{arm_id}_cont_has_existing"))?;
        self.builder
            .build_conditional_branch(has_existing, existing_bb, next_bb)?;

        self.builder.position_at_end(existing_bb);
        let existing_bb_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "escape continuation existing block",
                    at: span.into(),
                })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        let mut incoming: Vec<(PointerValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            vec![(existing, existing_bb_end)];

        for (index, site) in indirect_sites.iter().enumerate() {
            self.builder.position_at_end(next_bb);

            let token_index = frame_layout
                .ordinary_callee_resume_token_index(site.id())
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "escape continuation ordinary token index",
                    at: span.into(),
                })?;
            let escape_resume_state = site.escape_resume_state().unwrap_or(site.resume_state());
            let token_slot = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                token_index,
                &format!("site{}_arm_callee_resume_token_ptr", site.id()),
            )?;
            let token = self
                .builder
                .build_load(
                    self.llvm_gc_i8_ptr_type(),
                    token_slot,
                    &format!("site{}_arm_callee_resume_token", site.id()),
                )?
                .into_pointer_value();
            let has_token = self.ptr_is_non_null(
                span,
                token,
                &format!("site{}_arm_has_callee_resume_token", site.id()),
            )?;

            let materialize_bb = self.context.append_basic_block(
                current_fn,
                &format!("site{}_arm_cont_materialize", site.id()),
            );
            let else_bb = if index + 1 == indirect_sites.len() {
                self.context
                    .append_basic_block(current_fn, &format!("arm{arm_id}_cont_missing"))
            } else {
                self.context
                    .append_basic_block(current_fn, &format!("site{}_arm_cont_next", site.id()))
            };
            self.builder
                .build_conditional_branch(has_token, materialize_bb, else_bb)?;

            self.builder.position_at_end(materialize_bb);
            let dispatch_loop_fn_ptr = dispatch_loop_fn.as_global_value().as_pointer_value();
            let cont_alloc = self.declare_runtime_continuation_alloc();
            let cont = self
                .builder
                .build_call(
                    cont_alloc,
                    &[state_ptr.into(), dispatch_loop_fn_ptr.into()],
                    &format!("site{}_arm_continuation", site.id()),
                )?
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "arm continuation_alloc return value",
                    at: span.into(),
                })?
                .into_pointer_value();
            let deferred_cont = self.defer_gc_ref_pointer(
                span,
                &format!("site{}_arm_continuation_root", site.id()),
                cont,
            )?;
            let cont = self.reload_deferred_gc_ref_without_clearing(
                span,
                &format!("site{}_arm_continuation_reload", site.id()),
                &deferred_cont,
            )?;

            let cont_ty = self.llvm_continuation_struct_type();
            let cont_resume_state_gep = self.builder.build_struct_gep(
                cont_ty,
                cont,
                2,
                &format!("site{}_arm_cont_resume_state_tag", site.id()),
            )?;
            let resume_state_val = self
                .context
                .i32_type()
                .const_int(escape_resume_state as u64, false);
            self.builder
                .build_store(cont_resume_state_gep, resume_state_val)?;
            let set_captured =
                self.declare_runtime_continuation_set_captured_callee_suspend_state();
            self.builder.build_call(
                set_captured,
                &[cont.into(), token.into()],
                &format!("site{}_arm_set_captured_state", site.id()),
            )?;
            self.store_gc_ref_field(span, cont_gep, cont)?;
            let cont = self.reload_deferred_gc_ref_without_clearing(
                span,
                &format!("site{}_arm_continuation_return", site.id()),
                &deferred_cont,
            )?;
            self.clear_deferred_cg_value_root_homes(
                span,
                &format!("site{}_arm_continuation_drop", site.id()),
                &deferred_cont,
            )?;
            let materialize_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "escape continuation materialize block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;
            incoming.push((cont, materialize_bb_end));

            next_bb = else_bb;
        }

        self.builder.position_at_end(next_bb);
        if let Some(site) = direct_site {
            let dispatch_loop_fn_ptr = dispatch_loop_fn.as_global_value().as_pointer_value();
            let cont_alloc = self.declare_runtime_continuation_alloc();
            let cont = self
                .builder
                .build_call(
                    cont_alloc,
                    &[state_ptr.into(), dispatch_loop_fn_ptr.into()],
                    &format!("site{}_arm_direct_continuation", site.id()),
                )?
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "arm direct continuation_alloc return value",
                    at: span.into(),
                })?
                .into_pointer_value();
            let deferred_cont = self.defer_gc_ref_pointer(
                span,
                &format!("site{}_arm_direct_continuation_root", site.id()),
                cont,
            )?;
            let cont = self.reload_deferred_gc_ref_without_clearing(
                span,
                &format!("site{}_arm_direct_continuation_reload", site.id()),
                &deferred_cont,
            )?;
            let cont_ty = self.llvm_continuation_struct_type();
            let cont_resume_state_gep = self.builder.build_struct_gep(
                cont_ty,
                cont,
                2,
                &format!("site{}_arm_direct_cont_resume_state_tag", site.id()),
            )?;
            let resume_state_val = self.context.i32_type().const_int(
                site.escape_resume_state().unwrap_or(site.resume_state()) as u64,
                false,
            );
            self.builder
                .build_store(cont_resume_state_gep, resume_state_val)?;
            self.store_gc_ref_field(span, cont_gep, cont)?;
            let cont = self.reload_deferred_gc_ref_without_clearing(
                span,
                &format!("site{}_arm_direct_continuation_return", site.id()),
                &deferred_cont,
            )?;
            self.clear_deferred_cg_value_root_homes(
                span,
                &format!("site{}_arm_direct_continuation_drop", site.id()),
                &deferred_cont,
            )?;
            let direct_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "escape continuation direct materialize block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;
            incoming.push((cont, direct_bb_end));
        } else {
            let missing_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "escape continuation missing block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;
            incoming.push((self.llvm_gc_i8_ptr_type().const_null(), missing_bb_end));
        }

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(
            self.llvm_gc_i8_ptr_type(),
            &format!("arm{arm_id}_resolved_continuation"),
        )?;
        let refs: Vec<_> = incoming
            .iter()
            .map(|(ptr, bb)| (ptr as &dyn inkwell::values::BasicValue<'ctx>, *bb))
            .collect();
        phi.add_incoming(&refs);
        Ok(phi.as_basic_value().into_pointer_value())
    }

    /// Read a binder value from the TLS perform slot.
    fn read_binder_from_perform_slot(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.read_perform_slot_payload(at, cg_ty)
    }

    fn allocate_registered_handler_frames(
        &mut self,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<Vec<PointerValue<'ctx>>, LlvmEmitError> {
        let dispatch_entries = contract.dispatch_entries();
        if dispatch_entries.is_empty() {
            return Ok(Vec::new());
        }

        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_size = self.target_data.get_store_size(&handler_frame_ty);
        let handler_frame_size_val = self
            .llvm_ptr_sized_int_type(None)
            .const_int(handler_frame_size, false);
        let zero = self.context.i8_type().const_zero();
        let push_fn = self.declare_runtime_effect_handler_stack_push();

        let mut handler_frames = Vec::with_capacity(dispatch_entries.len());
        for (index, dispatch_entry) in dispatch_entries.iter().enumerate() {
            let handler_frame_ptr = self
                .builder
                .build_alloca(handler_frame_ty, &format!("handler_frame_{index}"))?;
            let handler_frame_i8 = self.builder.build_pointer_cast(
                handler_frame_ptr,
                self.llvm_i8_ptr_type(),
                &format!("handler_frame_{index}_i8"),
            )?;
            let _ = self
                .builder
                .build_memset(handler_frame_i8, 1, zero, handler_frame_size_val)?;

            let op_tag = self.effect_op_tag(dispatch_entry.op_fqn());
            let op_tag_val = self.context.i32_type().const_int(op_tag as u64, false);
            self.builder.build_call(
                push_fn,
                &[handler_frame_ptr.into(), op_tag_val.into()],
                &format!("push_handler_frame_{index}"),
            )?;
            handler_frames.push(handler_frame_ptr);
        }

        Ok(handler_frames)
    }

    fn pop_registered_handler_frames(
        &mut self,
        handler_frames: &[PointerValue<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        if handler_frames.is_empty() {
            return Ok(());
        }

        let pop_fn = self.declare_runtime_effect_handler_stack_pop();
        for (index, handler_frame_ptr) in handler_frames.iter().enumerate().rev() {
            self.builder.build_call(
                pop_fn,
                &[(*handler_frame_ptr).into()],
                &format!("pop_handler_frame_{index}"),
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::llvm::{
        build_main_module_from_lowered_hir, emit_minimal_main_ir,
        emit_minimal_main_ir_from_lowered_hir, run_pass_pipeline,
    };
    use crate::opt::OptLevel;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::{SourceFile, SourceMap};
    use crate::ty::TypeStore;
    use crate::typecheck;

    #[test]
    fn tail_resume_arm_body_rewrites_tail_resume_call_to_payload_expr() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    handle {
        Yield.next()
    } with {
        Yield.next(), k -> {
            println("in_handler")
            k.resume(41)
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind else {
            panic!("expected escape-continuation arm");
        };

        let rewritten = try_rewrite_tail_resume_arm_body(arm, continuation)
            .expect("tail-resume arm should rewrite");
        let hir::ExprKind::Block(block) = &rewritten.kind else {
            panic!("rewritten arm body should stay a block");
        };
        let Some(hir::Stmt {
            kind: hir::StmtKind::Expr(tail_expr),
            ..
        }) = block.stmts.last()
        else {
            panic!("rewritten block should keep an expr tail");
        };

        assert_eq!(source.slice(tail_expr.span), "41");
        assert!(matches!(tail_expr.kind, hir::ExprKind::Literal(_)));
    }

    #[test]
    fn tail_resume_arm_body_rewrites_if_branch_tails() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    handle {
        Yield.next()
    } with {
        Yield.next(), k -> {
            if (flag) {
                k.resume(1)
            } else {
                k.resume(2)
            }
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind else {
            panic!("expected escape-continuation arm");
        };

        let rewritten = try_rewrite_tail_resume_arm_body(arm, continuation)
            .expect("tail-resume arm should rewrite");
        let hir::ExprKind::Block(block) = &rewritten.kind else {
            panic!("rewritten arm body should stay a block");
        };
        let Some(hir::Stmt {
            kind: hir::StmtKind::Expr(tail_expr),
            ..
        }) = block.stmts.last()
        else {
            panic!("rewritten block should keep an expr tail");
        };
        let hir::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &tail_expr.kind
        else {
            panic!("rewritten tail should stay an if expression");
        };
        let else_branch = else_branch
            .as_ref()
            .expect("if tail should keep else branch");

        assert_eq!(source.slice(last_block_tail_expr(then_branch).span), "1");
        assert_eq!(source.slice(last_block_tail_expr(else_branch).span), "2");
    }

    #[test]
    fn tail_resume_arm_body_rewrites_non_block_tail_resume_call() {
        let (_, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    handle {
        Yield.next()
    } with {
        Yield.next(), k -> {
            println("in_handler")
            k.resume(41)
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind else {
            panic!("expected escape-continuation arm");
        };
        let hir::ExprKind::Block(block) = &arm.body.kind else {
            panic!("expected source arm body to lower to block");
        };
        let Some(hir::Stmt {
            kind: hir::StmtKind::Expr(tail_expr),
            ..
        }) = block.stmts.last()
        else {
            panic!("expected block arm body to keep a tail expr");
        };

        let direct_arm = hir::HandleArm {
            body: tail_expr.clone(),
            ..arm.clone()
        };
        let rewritten = try_rewrite_tail_resume_arm_body(&direct_arm, continuation)
            .expect("non-block tail-resume arm should rewrite");

        assert!(matches!(rewritten.kind, hir::ExprKind::Literal(_)));
    }

    #[test]
    fn escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Unit
}

fun main() {
    var saved: Continuation<Unit, Unit>? = None()
    var note: String = "before"

    val _: Unit = handle {
        val _pause: Unit = Suspend.pause()
        note = "after_resume"
    } with {
        Suspend.pause(), k -> {
            saved = Some(k)
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("seed_outer_slot_storage_"),
            "effect frame should record authoritative outer-slot storage pointers"
        );
        assert!(
            ir.contains("writeback_outer_slot_storage_ptr_"),
            "writeback path should load outer-slot storage pointers from the frame metadata"
        );
        assert!(
            ir.contains("writeback_outer_slot_storage_"),
            "writeback path should address the frame-recorded outer-slot storage metadata"
        );
    }

    #[test]
    fn state_machine_frame_slots_materialize_stable_exec_local_homes() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Query {
    fun query(): Int
}

fun main() {
    var saved: Continuation<Int, Unit>? = None()

    val _: Unit = handle {
        val _: Int = Query.query()
    } with {
        Query.query(), k -> {
            saved = Some(k)
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("handle_frame_home_saved"),
            "expected state-machine frame slot to materialize a stable exec local home alloca\n{ir}"
        );
        assert!(
            ir.contains(", ptr %handle_frame_home_saved"),
            "expected generated IR to use the exec local home (not a heap-frame GEP) as the env local home\n{ir}"
        );
    }

    #[test]
    fn suspend_ir_stores_callee_resume_token_on_frame_and_replays_via_resume_thunk() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Int
}

fun callIt(): Int / Suspend {
    Suspend.pause()
}

fun main(): Int {
    return handle {
        callIt()
    } with {
        Suspend.pause(), k -> {
            0
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("site0_callee_resume_token_ptr"),
            "suspend path should materialize a dedicated frame slot for the ordinary callee resume token"
        );
        assert!(
            ir.contains("site0_effect_outcome_effect_signal_resume_token"),
            "active suspend path should pull the ordinary callee resume token out of the explicit effect outcome"
        );
        assert!(
            ir.contains("@__scoop_callee_resume__a.callIt"),
            "ordinary callee should materialize a dedicated resume thunk instead of relying on TLS entry probing"
        );
        assert!(
            ir.contains("site0_call_callee_resume"),
            "resume replay path should call the stored resume thunk directly from the explicit frame token"
        );
        assert!(
            ir.contains("site0_captured_callee_resume_token")
                && ir.contains("scoop_continuation_set_captured_callee_suspend_state"),
            "fresh continuation materialization should capture the ordinary callee resume token so escaped continuation resume does not fall back to TLS"
        );
    }

    #[test]
    fn direct_perform_suspend_ir_uses_traceful_activation_hook() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Int
}

fun main(): Int {
    return handle {
        val value: Int = Suspend.pause()
        value
    } with {
        Suspend.pause(), k -> {
            0
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let marker = "val value: Int = Suspend.pause()";
        let offset = source
            .text()
            .find(marker)
            .and_then(|line_start| {
                source.text()[line_start..]
                    .find("Suspend.pause()")
                    .map(|rel| line_start + rel)
            })
            .expect("perform marker");
        let (line, col) = source.offset_to_line_col(offset).expect("perform line/col");
        let expected =
            format!("call void @scoop_effect_set_active_with_trace(i32 {line}, i32 {col})");
        assert!(
            ir.contains(&expected),
            "direct perform suspend site should publish traceful activation hook: {expected}\n{ir}"
        );
    }

    #[test]
    fn outer_suspend_does_not_reset_callee_trace_hook() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

fun boom(): Int / Raise<RuntimeError> {
    Raise.raise(RuntimeError.NullAssertionFailed)
    return 0
}

fun main(): Int {
    return try {
        boom()
    } catch (e: RuntimeError) {
        0
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let marker = "Raise.raise(RuntimeError.NullAssertionFailed)";
        let offset = source.text().find(marker).expect("raise marker");
        let (line, col) = source.offset_to_line_col(offset).expect("raise line/col");
        let expected =
            format!("call void @scoop_effect_set_active_with_trace(i32 {line}, i32 {col})");
        assert!(
            ir.contains(&expected),
            "callee perform path should preserve original raise trace hook: {expected}\n{ir}"
        );
        assert!(
            !ir.contains("call void @scoop_effect_set_active("),
            "outer suspend path should not reset active without trace and clobber the original raise-site metadata\n{ir}"
        );
    }

    #[test]
    fn escaped_continuation_ir_uses_dispatch_loop_entry_for_resume() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Int
}

fun main(): Int {
    return handle {
        Suspend.pause()
    } with {
        Suspend.pause(), k -> {
            0
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("define void @scoop.effect.dispatch."),
            "escaped continuation lowering should materialize a dedicated dispatch-loop entry"
        );
        let continuation_alloc_call = ir
            .lines()
            .find(|line| line.contains("call ") && line.contains("@scoop_continuation_alloc"))
            .expect("expected a continuation allocation call in IR");
        assert!(
            continuation_alloc_call.contains("@scoop.effect.dispatch."),
            "continuation allocation should capture the dispatch-loop entry instead of the raw step function"
        );
    }

    #[test]
    fn effect_runtime_functions_use_explicit_root_frame_without_statepoints() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Int
}

class Cell(var k: Continuation<Int, Unit>?)

fun main(): Int {
    val none_k: Continuation<Int, Unit>? = None()
    val cell: Cell = Cell(none_k)

    val _: Unit = handle {
        val _: Int = Suspend.pause()
    } with {
        Suspend.pause(), k -> {
            println("arm")
            cell.k = Some(k)
        }
    }

    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let context = inkwell::context::Context::create();
        let module = build_main_module_from_lowered_hir(
            &source_map,
            entry_source_id,
            &context,
            &lowered,
            None,
        )
        .expect("llvm module");
        let (target_machine, _target_info) =
            crate::llvm::target::host_target_machine_with_opt_level(OptLevel::O0)
                .expect("target machine");
        run_pass_pipeline(&module, &target_machine, OptLevel::O0).expect("run pass pipeline");
        let ir = module.print_to_string().to_string();

        let step_ir = find_function_ir(&ir, "define void @scoop.effect.step.");
        assert!(
            !step_ir.contains(r#"gc "statepoint-example""#)
                && !step_ir.contains("@llvm.experimental.gc.statepoint"),
            "default explicit mode should not leave effect step on the LLVM statepoint path"
        );

        let dispatch_ir = find_function_ir(&ir, "define void @scoop.effect.dispatch.");
        assert!(
            !dispatch_ir.contains(r#"gc "statepoint-example""#)
                && !dispatch_ir.contains("@llvm.experimental.gc.statepoint"),
            "default explicit mode should not leave effect dispatch on the LLVM statepoint path"
        );
    }

    #[test]
    fn escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(msg: String): Unit
}

class Cell(var saved: Continuation<Unit, Unit>?)

fun main(): Int {
    val none_k: Continuation<Unit, Unit>? = None()
    val cell: Cell = Cell(none_k)

    val _: Unit = handle {
        Suspend.pause("payload")
    } with {
        Suspend.pause(msg: String), k -> {
            cell.saved = Some(k)
        }
    }

    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let step_ir = find_function_ir(&ir, "define void @scoop.effect.step.");
        let switch_pos = step_ir
            .find("switch i32")
            .expect("expected step-function state dispatch switch");
        let continuation_alloca_pos = step_ir
            .find("cont_local = alloca ptr addrspace(1)")
            .expect("expected escape-arm fallback continuation local to spill via alloca");

        assert!(
            continuation_alloca_pos < switch_pos,
            "continuation spill slot must be created in the step-function entry block so statepoint rewriting can relocate it across later safepoints"
        );
        assert!(
            step_ir.contains("arm_binder_"),
            "escape-arm GC-ref binders should lower into traced effect-frame slots when the unified contract materializes a frame field"
        );
        assert!(
            step_ir.contains("binder_transport_gc_ref"),
            "escape-arm binder root contract should still read the GC-ref payload from the runtime perform slot before storing it into the frame"
        );
    }

    #[test]
    fn indirect_if_branch_callee_keeps_handle_call_site_active_dispatch() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): String
}

fun viaIf(flag: Bool, base: Int): String / (Ask) {
    val result: String = if (flag) do {
        println("if_enter")
        val inner: String = Ask.ask(base + 2)
        println("if_resume")
        println(inner)
        f"I:{inner}"
    } else do {
        "if_else"
    }
    println("if_after")
    println(result)
    result
}

fun main(): Int {
    val _: String = handle {
        viaIf(true, 20)
    } with {
        Ask.ask(seed), k -> {
            "fallback"
        }
    }
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");
        let step_ir = find_function_ir(&ir, "define void @scoop.effect.step.");
        let entry_block = find_block_ir(step_ir, "state_0");
        let fresh_call_block = find_block_ir(step_ir, "site0_suspend_call_fresh");
        let merge_block = find_block_ir(step_ir, "site0_suspend_call_merge");
        let active_block = find_block_ir(step_ir, "site0_active");

        assert!(
            entry_block.contains("site0_suspend_call_has_callee_resume_token")
                && entry_block.contains("site0_suspend_call_replay")
                && entry_block.contains("site0_suspend_call_fresh"),
            "outer handle should first branch between explicit callee replay and fresh call paths at the suspend site entry:\n{entry_block}"
        );
        assert!(
            fresh_call_block.contains("@__scoop_effect_call_wrapper__a.viaIf")
                && merge_block.contains("site0_effect_outcome_effect_outcome_tag")
                && !merge_block.contains("@scoop_effect_is_active"),
            "outer handle should route the fresh indirect call through the wrapper and then branch on the explicit outcome tag instead of probing TLS active again:\n{merge_block}"
        );
        assert!(
            !fresh_call_block.contains("@scoop_effect_is_active"),
            "fresh indirect call block itself must not reintroduce TLS active probing before the explicit outcome merge:\n{fresh_call_block}"
        );
        assert!(
            merge_block.contains("site0_effect_outcome_effect_outcome_is_propagating"),
            "outer handle merge path should branch on the explicit outcome produced by the indirect if-branch callee call:\n{merge_block}"
        );
        assert!(
            active_block.contains("@scoop_effect_outcome_publish"),
            "outer handle should still preserve the active-dispatch path for the indirect if-branch callee call by publishing the captured outcome back to TLS:\n{active_block}"
        );
        assert!(
            ir.contains("@__scoop_callee_resume__a.viaIf"),
            "ordinary if-branch callee should now expose a dedicated resume thunk instead of a TLS-switched dual entry"
        );
    }

    #[test]
    fn direct_suspend_call_fresh_path_uses_explicit_outcome_instead_of_tls_probe() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun helper(): Int / (Ask) {
    Ask.ask(41)
}

fun main(): Int {
    val result: Int = handle {
        helper()
    } with {
        Ask.ask(seed), k -> {
            seed + 1
        }
    }
    return result
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");
        let step_ir = find_function_ir(&ir, "define void @scoop.effect.step.");
        let entry_block = find_block_ir(step_ir, "state_0");
        let fresh_call_block = find_block_ir(step_ir, "site0_suspend_call_fresh");
        let merge_block = find_block_ir(step_ir, "site0_suspend_call_merge");
        let active_block = find_block_ir(step_ir, "site0_active");

        assert!(
            entry_block.contains("site0_suspend_call_has_callee_resume_token")
                && entry_block.contains("site0_suspend_call_replay")
                && entry_block.contains("site0_suspend_call_fresh"),
            "state-machine suspend-call entry should branch between explicit ordinary replay and fresh wrapper evaluation:\n{entry_block}"
        );
        assert!(
            fresh_call_block.contains("@__scoop_effect_call_wrapper__a.helper")
                && !fresh_call_block.contains("@scoop_effect_is_active"),
            "state-machine fresh suspend-call block should route through the wrapper without probing TLS active directly:\n{fresh_call_block}"
        );
        assert!(
            merge_block.contains("site0_effect_outcome_effect_outcome_tag")
                && !merge_block.contains("@scoop_effect_is_active"),
            "state-machine suspend-call merge should branch on the explicit outcome tag produced by the wrapper instead of probing TLS active again:\n{merge_block}"
        );
        assert!(
            active_block.contains("@scoop_effect_outcome_publish"),
            "state-machine active suspend branch must publish the explicit outcome back to TLS before dispatching handlers:\n{active_block}"
        );
        assert!(
            ir.contains("@__scoop_callee_resume__a.helper")
                && ir.contains("site0_call_callee_resume"),
            "ordinary callee replay should now route through the explicit helper resume thunk and frame token path"
        );
    }

    #[test]
    fn ordinary_multi_site_callee_materializes_resume_site_dispatch() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): String
}

fun viaBranch(flag: Bool, base: Int): String / (Ask) {
    val result: String = if (flag) do {
        val inner: String = Ask.ask(base + 1)
        f"T:{inner}"
    } else do {
        val inner: String = Ask.ask(base + 2)
        f"F:{inner}"
    }
    result
}

fun main(): Int {
    val _: String = handle {
        viaBranch(true, 10)
    } with {
        Ask.ask(seed), k -> {
            "fallback"
        }
    }
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("callee_resume_site_tag"),
            "multi-site ordinary callee resume path should read the saved resume site tag"
        );
        assert!(
            ir.contains("resume_site0"),
            "multi-site ordinary callee should materialize a dedicated resume block for site0"
        );
        assert!(
            ir.contains("resume_site1"),
            "multi-site ordinary callee should materialize a dedicated resume block for site1"
        );
    }

    #[test]
    fn ordinary_callee_resume_site_drops_unreachable_suffix_after_nested_return() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): Int
}

fun helper(flag: Bool): Int / (Ask) {
    println("helper_before")
    if (flag) {
        println("helper_suspend")
        return Ask.ask()
    }

    println("helper_direct")
    println("helper_after")
    return 7
}

fun main(): Int {
    val result: Int = handle {
        val value: Int = helper(true) + 1
        value
    } with {
        Ask.ask(), k -> {
            k.resume(2)
        }
    }
    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let resume_site_ir = find_block_ir(&ir, "resume_site0");
        let (_, after_return) = resume_site_ir
            .split_once("br label %return")
            .expect("resume site should branch to return");

        assert!(
            after_return.trim().is_empty(),
            "ordinary callee resume-site block must end at the first return branch instead of appending unreachable suffix:\n{resume_site_ir}"
        );
    }

    #[test]
    fn ordinary_callee_resume_site_drops_unreachable_suffix_after_when_all_arms_return() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): Int
}

fun helper(flag: Bool): Int / (Ask) {
    println("helper_before")
    when (flag) {
        true -> {
            println("helper_suspend")
            return Ask.ask()
        }
        false -> {
            println("helper_direct")
            return 7
        }
    }

    println("helper_after")
    return 9
}

fun main(): Int {
    val result: Int = handle {
        val value: Int = helper(true) + 1
        value
    } with {
        Ask.ask(), k -> {
            k.resume(2)
        }
    }
    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let helper_ir = find_function_ir(&ir, "define i64 @__scoop_callee_resume__a.helper");
        let resume_site_ir = find_block_ir(helper_ir, "resume_site0");
        let (_, after_return) = resume_site_ir
            .split_once("br label %return")
            .expect("all-returning when on resumed path should jump directly to return");

        assert!(
            after_return.trim().is_empty(),
            "ordinary callee when-arm resumed tail must stop at the first return branch instead of appending unreachable suffix:\n{resume_site_ir}"
        );
    }

    #[test]
    fn runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

open class Base()
class Impl() : Base()

fun main(): Int {
    val x: Any = Impl()

    val _: Unit = try {
        val _b: Base = x as Base
        println("after_cast")
    } catch (e: RuntimeError) {
        println("caught")
    }

    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        assert!(
            ir.contains("site0_is_active"),
            "runtime raise boundary should check TLS active before deciding whether to continue"
        );
        assert!(
            ir.contains("site0_inactive"),
            "runtime raise boundary should keep the inactive success path in the current state machine"
        );
        assert!(
            ir.contains("site0_active"),
            "runtime raise boundary should still preserve the active outward-dispatch path"
        );
    }

    #[test]
    fn runtime_raise_boundary_ir_preserves_runtime_error_variant_payload() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

open class Base()
class Impl() : Base()
class Other() : Base()

fun main(): Int {
    val x: Any = Impl()

    val result: Int = try {
        val _o: Other = x as Other
        0
    } catch (e: RuntimeError) {
        when (e) {
            NullAssertionFailed -> 1
            ClassCastFailed -> 2
            ContinuationAlreadyResumed -> 3
        }
    }

    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        let payload_call = ir
            .lines()
            .find(|line| {
                line.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref")
                    && (line.contains("i32 -1") || line.contains("i32 4294967295"))
            })
            .expect("expected runtime-error raise payload write in IR");

        assert!(
            payload_call.contains("i64 1"),
            "ClassCastFailed should be transported as its concrete RuntimeError variant tag, not a collapsed zero payload: {payload_call}"
        );
    }

    #[test]
    fn multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

effect Alpha {
    fun fail(code: Int): Nothing
}

effect Beta {
    fun stop(msg: String): Nothing
}

fun main(): Int {
    return handle {
        0
    } with {
        Alpha.fail(code: Int) -> {
            code
        }
        Beta.stop(msg: String) -> {
            0
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        let push_count = ir
            .matches("call void @scoop_effect_handler_stack_push")
            .count();
        let pop_count = ir
            .matches("call void @scoop_effect_handler_stack_pop")
            .count();

        assert_eq!(
            push_count, 2,
            "multi-op handle should push one runtime handler frame per dispatch entry"
        );
        assert_eq!(
            pop_count, 4,
            "each registered handler frame should be popped on both done and propagate exits"
        );
        assert!(
            ir.contains("handler_frame_0"),
            "IR should materialize a dedicated runtime handler frame for the first op"
        );
        assert!(
            ir.contains("handler_frame_1"),
            "IR should materialize a dedicated runtime handler frame for the second op"
        );
    }

    #[test]
    fn async_task_resume_ir_does_not_replay_original_await_site() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async {
        println("before")
        val x: Int = await __task_from_result(41)
        println("after")
        println(x)
        x + 1
    }
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        let async_closure_ir = ir
            .split("\ndefine ")
            .skip(1)
            .find_map(|chunk| {
                let function = format!("define {chunk}");
                let body_end = function.find("\n}")?;
                let function = &function[..body_end + 2];
                (function.contains("scoop.core.__task_step_pending::<")
                    && function.contains("scoop.core.__task_step_ready::<"))
                .then_some(function.to_string())
            })
            .expect("expected async task closure function in IR");

        assert_eq!(
            async_closure_ir
                .matches("@scoop_effect_perform_slot_write_u64_with_gc_ref")
                .count(),
            1,
            "resumed async task body must not replay the original await perform site:\n{async_closure_ir}"
        );
        assert_eq!(
            async_closure_ir
                .matches("scoop.core.__task_step_pending::<")
                .count(),
            1,
            "async task closure should materialize exactly one pending step helper for the single await site:\n{async_closure_ir}"
        );
        assert_eq!(
            async_closure_ir
                .matches("scoop.core.__task_step_ready::<")
                .count(),
            1,
            "async task closure should materialize exactly one ready step helper on normal completion:\n{async_closure_ir}"
        );
    }

    #[test]
    fn async_task_pending_path_stores_escape_continuation_before_waiting_helper() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val inner: Task<Int> = async { 41 }
    val outer: Task<Int> = async {
        val x: Int = await inner
        x + 1
    }
    when (outer.step()) {
        TaskStep.Pending -> 0
        TaskStep.Ready(value) -> value
    }
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");
        let async_closure_ir = ir
            .split("\ndefine ")
            .skip(1)
            .find_map(|chunk| {
                let function = format!("define {chunk}");
                let body_end = function.find("\n}")?;
                let function = &function[..body_end + 2];
                (function.contains("scoop.core.__task_step_pending::<")
                    && function.contains("load_continuation")
                    && function.contains("continuation_val"))
                .then_some(function.to_string())
            })
            .expect("expected async task closure function in IR");
        let load_idx = async_closure_ir
            .find("%load_continuation =")
            .expect("await pending path should load escaped continuation from the effect frame");
        let pending_idx = async_closure_ir
            .find("call void @\"scoop.core.__task_step_pending::<Int>\"")
            .expect("await pending path should call __task_step_pending helper");
        let pending_window = &async_closure_ir[load_idx..pending_idx];

        assert!(
            pending_window.contains("store ptr addrspace(1) %continuation_val, ptr %cont_local")
                && pending_window.contains("ptr %explicit_root_frame_slot_"),
            "await pending path must store the escaped continuation into its tracked local and explicit-frame home slot before calling __task_step_pending, otherwise waiting resumes will see null continuation:\n{async_closure_ir}"
        );
    }

    #[test]
    fn async_task_resume_replay_ir_terminates_step_fn_on_active_effect() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async {
        println("before")
        val x: Int = await __task_from_result(41)
        println("after")
        println(x)
        x + 1
    }
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");
        let async_closure_ir = ir
            .split("\ndefine ")
            .skip(1)
            .find_map(|chunk| {
                let function = format!("define {chunk}");
                let body_end = function.find("\n}")?;
                let function = &function[..body_end + 2];
                (function.contains("scoop.core.__task_step_pending::<")
                    && function.contains("scoop.core.__task_step_ready::<"))
                .then_some(function.to_string())
            })
            .expect("expected async task closure function in IR");
        let replay_block = async_closure_ir
            .split("site0_resume_replay:")
            .nth(1)
            .and_then(|tail| tail.split("site0_resume_inactive:").next())
            .expect("expected resume replay block in async task closure IR");
        let active_return_block = async_closure_ir
            .split("site0_callee_resume_return:")
            .nth(1)
            .and_then(|tail| tail.split("site0_callee_resume_continue:").next())
            .expect("expected active-effect early return block in async task closure IR");
        let continue_block = find_block_ir(&async_closure_ir, "site0_callee_resume_continue");

        assert!(
            replay_block.contains("site0_callee_resume_effect_outcome_tag")
                && replay_block.contains("site0_callee_resume_effect_outcome_is_propagating")
                && !replay_block.contains("@scoop_effect_is_active"),
            "replayed ordinary callee inside async task should branch directly on the explicit outcome instead of probing TLS active or materializing a fallback answer:\n{async_closure_ir}"
        );
        assert!(
            active_return_block.contains("ret void"),
            "active resume replay path should terminate the step function immediately:\n{async_closure_ir}"
        );
        assert!(
            continue_block.contains("resume_slot_")
                && continue_block.contains("@scoop_gc_write_barrier")
                && continue_block.contains("ptr addrspace(1) %site0_call_callee_resume")
                && continue_block.contains("br label %site0_resume_merge"),
            "inactive resume replay path should stash the replayed answer into the synthetic resume slot before rejoining the state machine:\n{async_closure_ir}"
        );
    }

    #[test]
    fn when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun pause(): Int
}

class Cell(var k: Continuation<Int, Unit>?)

fun main(): Int {
    val none_k: Continuation<Int, Unit>? = None()
    val cell: Cell = Cell(none_k)

    val result: Int = try {
        val _: Unit = handle {
            val _: Int = Suspend.pause()
        } with {
            Suspend.pause(), k -> {
                cell.k = Some(k)
            }
        }

        when (cell.k) {
            Some(k1) -> {
                cell.k = none_k
                val _: Unit = try {
                    k1.resume(10)
                } catch (e: RuntimeError) {
                    println("resume_err")
                }
            }
            None -> println("missing")
        }

        0
    } catch (outer: RuntimeError) {
        1
    }

    return result
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("call i32 @scoop_continuation_resume_with"),
            "outer when-arm try/catch should lower Continuation.resume via the shared payload+answer runtime entry"
        );
        assert!(
            ir.contains("continuation_resume_replay_token"),
            "Continuation.resume replay should persist the explicit resume_token on the frame instead of relying on callee-state replay bookkeeping"
        );
        assert!(
            !ir.contains("continuation_resume_replay_state_raw"),
            "Continuation.resume replay should no longer materialize the legacy TLS replay-state reader"
        );
    }

    #[test]
    fn continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): String
}

class Cell(var k: Continuation<String, Unit>?)

fun main(): Int {
    val none_k: Continuation<String, Unit>? = None()
    val cell: Cell = Cell(none_k)

    val _: Unit = handle {
        val _: String = Ask.ask()
    } with {
        Ask.ask(), k -> {
            cell.k = Some(k)
        }
    }

    when (cell.k) {
        Some(k1) -> {
            cell.k = none_k
            val _: Unit = try {
                k1.resume("alpha")
            } catch (e: RuntimeError) {
                println("resume_err")
            }
        }
        None -> println("missing")
    }

    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");
        let resume_step_ir = ir
            .split("\ndefine ")
            .skip(1)
            .find_map(|chunk| {
                let function = format!("define {chunk}");
                let body_end = function.find("\n}")?;
                let function = &function[..body_end + 2];
                (function.contains("@scoop_continuation_resume_with")
                    && function.contains("continuation_resume_receiver_reload"))
                .then_some(function.to_string())
            })
            .expect("expected nested continuation resume step function in IR");
        let state_0 = find_block_ir(&resume_step_ir, "state_0");
        let payload_alloc_idx = state_0
            .find("call ptr addrspace(1) @scoop_alloc_typed")
            .expect("expected String payload allocation before resume");
        let receiver_reload_idx = state_0
            .find("continuation_resume_receiver_reload = load ptr addrspace(1)")
            .expect("expected continuation receiver reload");
        let resume_call_idx = state_0
            .find("call i32 @scoop_continuation_resume_with")
            .expect("expected continuation resume runtime call");

        assert!(
            payload_alloc_idx < receiver_reload_idx && receiver_reload_idx < resume_call_idx,
            "Continuation.resume must reload the receiver after GC-sensitive payload materialization and before the runtime resume call, otherwise GC-stress may pass a stale continuation pointer:\n{state_0}"
        );
    }

    #[test]
    fn continuation_resume_boxed_payload_reloads_box_object_before_runtime_call() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

struct Named(val name: String, val score: Int)

effect GetNamed {
    fun get(): Named
}

fun main(): Int {
    var saved: Continuation<Named, Unit>? = None()

    val _: Unit = handle {
        val _: Named = GetNamed.get()
    } with {
        GetNamed.get(), k -> {
            saved = Some(k)
        }
    }

    when (saved) {
        Some(k) -> {
            val _: Unit = try {
                k.resume(Named { name: "alice", score: 42 })
            } catch (e: RuntimeError) {
                println("resume_err")
            }
        }
        None -> println("missing")
    }

    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");
        let resume_step_ir = ir
            .split("\ndefine ")
            .skip(1)
            .find_map(|chunk| {
                let function = format!("define {chunk}");
                let body_end = function.find("\n}")?;
                let function = &function[..body_end + 2];
                (function.contains("@scoop_continuation_resume_with")
                    && function.contains("effect_transport_box_obj_reload"))
                .then_some(function.to_string())
            })
            .expect("expected continuation resume function with boxed payload in IR");
        let state_0 = find_block_ir(&resume_step_ir, "state_0");
        let payload_box_alloc_idx = state_0
            .find("rt_alloc_effect_value_box = call ptr addrspace(1) @scoop_alloc_typed")
            .expect("expected boxed payload allocation before resume");
        let box_reload_idx = state_0
            .find("effect_transport_box_obj_reload = load ptr addrspace(1)")
            .expect("expected boxed payload object reload");
        let resume_call_idx = state_0
            .find("call i32 @scoop_continuation_resume_with")
            .expect("expected continuation resume runtime call");

        assert!(
            payload_box_alloc_idx < box_reload_idx && box_reload_idx < resume_call_idx,
            "Continuation.resume boxed payload path must reload the freshly allocated transport box before the runtime resume call, otherwise GC-stress may pass a stale boxed payload pointer:\n{state_0}"
        );
    }

    #[test]
    fn non_resuming_arm_ir_does_not_publish_pending_continuation() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): Int
}

effect Abort {
    fun stop(): Nothing
}

class Cell(var k: Continuation<Int, Int>?)

fun main(): Int {
    val none_k: Continuation<Int, Int>? = None()
    val cell: Cell = Cell(none_k)

    return handle {
        val first: Int = Ask.ask()
        if (first > 0) {
            Abort.stop()
        } else {
            0
        }
    } with {
        Ask.ask(), k -> {
            cell.k = Some(k)
            7
        }
        Abort.stop() -> {
            9
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        let publish_calls = ir
            .lines()
            .filter(|line| {
                line.contains("@scoop_continuation_resume_publish_pending_continuation")
                    && line.contains("call")
            })
            .count();
        assert_eq!(
            publish_calls, 1,
            "only escape-continuation materialization should publish outer replay pending continuation, got {publish_calls} publish sites:\n{ir}"
        );
    }

    #[test]
    fn same_op_multi_arm_dispatch_ir_reads_effect_instance_key() {
        let (source, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

open class Base()
class Sub() : Base()

fun raiseSub(): Int / Raise<Sub> {
    val err: Sub = Sub()
    Raise.raise(err)
}

fun main(): Int {
    return handle {
        raiseSub()
    } with {
        Raise.raise(err: Sub) -> {
            1
        }
        Raise.raise(err: Base) -> {
            2
        }
    }
}
"#,
        );
        let session = Session::new().expect("session");
        let mut source_map = SourceMap::default();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let entry_source_id = source_map.add_source_clone(&source);
        let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered)
            .expect("llvm ir");

        assert!(
            ir.contains("@scoop_effect_perform_slot_read_effect_instance_key"),
            "same-op multi-arm dispatch should read effect_instance_key from the perform slot"
        );
        assert!(
            ir.contains("dispatch_arm_0_check"),
            "first same-op arm should have an explicit effect-instance check block"
        );
        assert!(
            ir.contains("dispatch_arm_1_check"),
            "later same-op sibling arm should keep its own effect-instance check block"
        );
        assert!(
            ir.contains("arm_0_effect_instance_match_0"),
            "same-op dispatch should generate an effect-instance compare for the first arm"
        );
        assert!(
            ir.contains("arm_1_effect_instance_match_0"),
            "same-op dispatch should generate an effect-instance compare for later sibling arms"
        );
    }

    #[test]
    fn cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun main(): Int {
    val result: Int = handle {
        42
    } with {
        Yield.next() -> 0
    } finally {
        println("cleanup")
    }

    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        assert!(
            ir.contains("cleanup_enter_already_ran"),
            "CleanupEnter lowering should branch on the persisted cleanup flag before reentering the cleanup scope"
        );
    }

    #[test]
    fn cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun main(): Int {
    val result: Int = handle {
        42
    } with {
        Yield.next() -> 0
    } finally {
        println("cleanup")
    }

    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        assert!(
            ir.contains("cleanup_propagate_pre_state_tag"),
            "cleanup propagate path should preserve the pre-cleanup state tag before entering shared finally"
        );
        assert!(
            ir.contains("cleanup_propagate_restore_propagating_state_terminal_state"),
            "cleanup propagate path should detect shared finally exits that leak terminal sentinels"
        );
        assert!(
            ir.contains("cleanup_propagate_restore_propagating_state_value"),
            "cleanup propagate path should restore the propagating state instead of leaving a terminal completion tag behind"
        );
    }

    #[test]
    fn dispatch_loop_ir_checks_terminal_state_before_tls_active() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun main(): Int {
    val result: Int = handle {
        42
    } with {
        Yield.next() -> 0
    } finally {
        println("cleanup")
    }

    println(result)
    return 0
}
"#,
        );
        let session = Session::new().expect("session");
        let ir = emit_minimal_main_ir(&session, &source).expect("llvm ir");

        let terminal_check_pos = ir
            .find("dispatch_terminal_state_0_eq")
            .expect("dispatch loop should compare state_tag against terminal sentinels");
        let active_check_pos = ir
            .find("handle_dispatch_is_active")
            .expect("dispatch loop should still read TLS active after terminal-state check");
        assert!(
            terminal_check_pos < active_check_pos,
            "dispatch loop must prefer terminal state_tag over TLS active when deciding done vs dispatch"
        );
        assert!(
            ir.contains("dispatch_active_check"),
            "dispatch loop should keep a dedicated active-check block after the terminal-state guard"
        );
    }

    #[test]
    fn typed_lowering_preserves_raise_helper_performed_effect_instance() {
        let (_, lowered) = lower_typed_single_source_with_source(
            r#"
package a

import scoop.core.*

open class Base()
class Sub() : Base()

fun raiseSub(): Int / Raise<Sub> {
    val err: Sub = Sub()
    Raise.raise(err)
}
"#,
        );

        let perform = first_perform_in_fun(&lowered.file, "raiseSub").expect("expected perform");
        let hir::ExprKind::Perform { effect_ty, .. } = perform.kind else {
            panic!("expected perform expr");
        };
        let effect_text = lowered.types.display(effect_ty).to_string();
        assert!(
            effect_text.contains("Raise") && effect_text.contains("Sub"),
            "expected performed effect instance to mention Raise<Sub>, got {effect_text}"
        );
    }

    fn lower_typed_single_source_with_source(source_text: &str) -> (SourceFile, hir::LoweredHir) {
        let session = Session::new().expect("session");
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).expect("parse");

        let index = {
            let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).expect("index")
        };

        let headers =
            crate::resolve::check_file_headers(&source, &ast, &index).expect("resolve headers");
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers)
            .expect("resolve bodies");

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).expect("env");
        env.extend_from_file(&source, &ast, &index)
            .expect("extend type env");

        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check annotations");
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check type refs");
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check exprs");

        let mut unit: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        let lowered = hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .expect("lower");
        (source, lowered)
    }

    fn first_handle_in_file(file: &hir::File) -> Option<(&hir::FunDecl, &hir::HandleExpr)> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(body) = &fun.body
                && let Some(handle) = first_handle_in_block(body)
            {
                return Some((fun, handle));
            }
        }
        None
    }

    fn first_handle_in_block(block: &hir::Block) -> Option<&hir::HandleExpr> {
        for stmt in &block.stmts {
            if let Some(handle) = first_handle_in_stmt(stmt) {
                return Some(handle);
            }
        }
        None
    }

    fn first_handle_in_stmt(stmt: &hir::Stmt) -> Option<&hir::HandleExpr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_handle_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_handle_in_expr(cond).or_else(|| first_handle_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    fn first_handle_in_expr(expr: &hir::Expr) -> Option<&hir::HandleExpr> {
        match &expr.kind {
            hir::ExprKind::Handle(handle) => Some(handle),
            hir::ExprKind::Block(block) => first_handle_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_handle_in_expr(cond)
                .or_else(|| first_handle_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_handle_in_expr)),
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter()
                    .find_map(|arm| arm.guard.as_ref().and_then(first_handle_in_expr))
                    .or_else(|| arms.iter().find_map(|arm| first_handle_in_expr(&arm.body)))
            }),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::MemberAccess { receiver: expr, .. } => first_handle_in_expr(expr),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::ExprKind::Call { callee, args } => first_handle_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
                })
            }),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
            }),
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| first_handle_in_expr(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_handle_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                first_handle_in_expr(expr)
            }),
            hir::ExprKind::Closure(closure) => first_handle_in_expr(&closure.body),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn first_perform_in_fun<'a>(file: &'a hir::File, name: &str) -> Option<&'a hir::Expr> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && fun.name == name
                && let Some(body) = &fun.body
                && let Some(expr) = first_perform_in_block(body)
            {
                return Some(expr);
            }
        }
        None
    }

    fn first_perform_in_block(block: &hir::Block) -> Option<&hir::Expr> {
        for stmt in &block.stmts {
            if let Some(expr) = first_perform_in_stmt(stmt) {
                return Some(expr);
            }
        }
        None
    }

    fn first_perform_in_stmt(stmt: &hir::Stmt) -> Option<&hir::Expr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_perform_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_perform_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_perform_in_expr(lhs).or_else(|| first_perform_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_perform_in_expr(cond).or_else(|| first_perform_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_perform_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    fn find_function_ir<'a>(ir: &'a str, prefix: &str) -> &'a str {
        let start = ir.find(prefix).expect("expected function definition");
        let rest = &ir[start..];
        let end = rest.find("\ndefine ").unwrap_or(rest.len());
        &rest[..end]
    }

    fn find_block_ir<'a>(function_ir: &'a str, label: &str) -> &'a str {
        let needle = format!("{label}:");
        let start = function_ir.find(&needle).expect("expected block label");
        let rest = &function_ir[start..];
        let mut offset = 0usize;
        let mut end = rest.len();
        for (index, line) in rest.split_inclusive('\n').enumerate() {
            if index > 0 && (is_ir_block_label(line) || is_ir_block_boundary(line)) {
                end = offset;
                break;
            }
            offset += line.len();
        }
        &rest[..end]
    }

    fn is_ir_block_label(line: &str) -> bool {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
            return false;
        }
        let Some(colon_idx) = line.find(':') else {
            return false;
        };
        line[..colon_idx]
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    }

    fn is_ir_block_boundary(line: &str) -> bool {
        let line = line.trim();
        line == "}" || line.starts_with("define ") || line.starts_with("declare ")
    }

    fn first_perform_in_expr(expr: &hir::Expr) -> Option<&hir::Expr> {
        match &expr.kind {
            hir::ExprKind::Perform { .. } => Some(expr),
            hir::ExprKind::Block(block) => first_perform_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_perform_in_expr(cond)
                .or_else(|| first_perform_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_perform_in_expr)),
            hir::ExprKind::When { subject, arms } => first_perform_in_expr(subject).or_else(|| {
                arms.iter()
                    .find_map(|arm| arm.guard.as_ref().and_then(first_perform_in_expr))
                    .or_else(|| arms.iter().find_map(|arm| first_perform_in_expr(&arm.body)))
            }),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::MemberAccess { receiver: expr, .. } => first_perform_in_expr(expr),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_perform_in_expr(lhs).or_else(|| first_perform_in_expr(rhs))
            }
            hir::ExprKind::Call { callee, args } => first_perform_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_perform_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_perform_in_expr(value),
                })
            }),
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| first_perform_in_expr(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_perform_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                first_perform_in_expr(expr)
            }),
            hir::ExprKind::Handle(handle) => first_perform_in_block(&handle.body)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| first_perform_in_expr(&arm.body))
                })
                .or_else(|| handle.finally.as_ref().and_then(first_perform_in_block)),
            hir::ExprKind::Closure(closure) => first_perform_in_expr(&closure.body),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn last_block_tail_expr(expr: &hir::Expr) -> &hir::Expr {
        let hir::ExprKind::Block(block) = &expr.kind else {
            panic!("expected block expression");
        };
        let Some(hir::Stmt {
            kind: hir::StmtKind::Expr(tail_expr),
            ..
        }) = block.stmts.last()
        else {
            panic!("expected block tail expression");
        };
        tail_expr
    }
}
