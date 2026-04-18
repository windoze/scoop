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

use super::unified_state_machine_skeleton::FrameSlot;
use super::*;

use super::unified_state_machine_skeleton::{
    HandleBranchCondition, HandleStateOp, ResumeAfterSiteReason, SuspendSiteKind, UnifiedArm,
    UnifiedFrameField, UnifiedFrameSystemField, UnifiedHandleLoweringContract, UnifiedState,
    UnifiedStateContext, UnifiedStateTerminator,
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

fn rewrite_immediate_resume_arm_body(
    arm: &hir::HandleArm,
    resume_symbol: hir::SymbolId,
) -> Result<hir::Expr, LlvmEmitError> {
    // Internal lowerings such as `await task` may synthesize an immediate-
    // resume arm whose body is a direct `resume(expr)` call rather than a
    // source-level block. Rewrite from the top-level tail expression so both
    // source-authored blocks and synthesized expression bodies share the same
    // dedicated path.
    rewrite_immediate_resume_tail_expr(&arm.body, resume_symbol)
}

fn rewrite_immediate_resume_tail_stmt(
    stmt: &mut hir::Stmt,
    resume_symbol: hir::SymbolId,
) -> Result<hir::Expr, LlvmEmitError> {
    let hir::StmtKind::Expr(expr) = &mut stmt.kind else {
        return Err(LlvmEmitError::UnsupportedMainBody {
            kind: "immediate resume arm tail statement",
            at: stmt.span.into(),
        });
    };
    let rewritten = rewrite_immediate_resume_tail_expr(expr, resume_symbol)?;
    stmt.ty = rewritten.ty;
    *expr = rewritten.clone();
    Ok(rewritten)
}

fn rewrite_immediate_resume_tail_expr(
    expr: &hir::Expr,
    resume_symbol: hir::SymbolId,
) -> Result<hir::Expr, LlvmEmitError> {
    if let Some(payload) = extract_immediate_resume_payload_expr(expr, resume_symbol)? {
        return Ok(payload);
    }

    match &expr.kind {
        hir::ExprKind::Block(block) => {
            let mut rewritten_block = block.clone();
            let Some(tail_stmt) = rewritten_block.stmts.last_mut() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "immediate resume nested block tail",
                    at: expr.span.into(),
                });
            };
            let rewritten_tail = rewrite_immediate_resume_tail_stmt(tail_stmt, resume_symbol)?;
            rewritten_block.ty = rewritten_tail.ty;
            Ok(hir::Expr {
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
            let rewritten_then = rewrite_immediate_resume_tail_expr(then_branch, resume_symbol)?;
            let rewritten_else =
                else_branch
                    .as_ref()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "immediate resume if tail without else",
                        at: expr.span.into(),
                    })?;
            let rewritten_else = rewrite_immediate_resume_tail_expr(rewritten_else, resume_symbol)?;
            Ok(hir::Expr {
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
                arm.body = rewrite_immediate_resume_tail_expr(&arm.body, resume_symbol)?;
            }
            let result_ty = rewritten_arms
                .first()
                .map(|arm| arm.body.ty)
                .unwrap_or(expr.ty);
            Ok(hir::Expr {
                span: expr.span,
                ty: result_ty,
                kind: hir::ExprKind::When {
                    subject: subject.clone(),
                    arms: rewritten_arms,
                },
            })
        }
        _ => Err(LlvmEmitError::UnsupportedMainBody {
            kind: "immediate resume arm tail expression",
            at: expr.span.into(),
        }),
    }
}

fn extract_immediate_resume_payload_expr(
    expr: &hir::Expr,
    resume_symbol: hir::SymbolId,
) -> Result<Option<hir::Expr>, LlvmEmitError> {
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        return Ok(None);
    };
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind else {
        return Ok(None);
    };
    if *id != resume_symbol {
        return Ok(None);
    }

    let payload = match args.as_slice() {
        [hir::CallArg::Positional(payload)] => payload.clone(),
        [hir::CallArg::Named { value, .. }] => value.clone(),
        _ => {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "immediate resume call arity",
                at: expr.span.into(),
            });
        }
    };

    Ok(Some(payload))
}

/// Tracks the frame struct layout for a specific handle expression, mapping
/// `UnifiedFrameField` indices to LLVM struct field indices.
pub(super) struct FrameLayout<'ctx> {
    pub(super) frame_type: inkwell::types::StructType<'ctx>,
    cleanup_flag_index: Option<u32>,
    completion_tag_index: Option<u32>,
    continuation_index: u32,
    outer_scope_storage_indices: HashMap<hir::SymbolId, u32>,
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
        let mut user_slots: Vec<(usize, crate::ty::TypeId)> = frame
            .slots()
            .iter()
            .map(|slot| (slot.field_index(), slot.slot().ty()))
            .collect();
        user_slots.sort_by_key(|(idx, _)| *idx);

        for (_idx, type_id) in &user_slots {
            let cg_ty = self
                .cg_ty_of(*type_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect frame slot type",
                    at: span.into(),
                })?;
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
        let saved_env = std::mem::take(&mut self.env);
        let saved_return_ctx = self.return_context.take();
        let saved_return_ty = self.current_fun_return_ty.take();
        let saved_callee_suspend_plan = self.current_callee_suspend_plan.take();
        let saved_loop_stack = std::mem::take(&mut self.loop_context_stack);
        let enclosing_return_ty = self
            .effect_function_return_context
            .map(|ctx| ctx.return_ty)
            .or(saved_return_ty);

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
        self.loop_context_stack = saved_loop_stack;
        self.current_callee_suspend_plan = saved_callee_suspend_plan;
        self.current_fun_return_ty = saved_return_ty;
        self.return_context = saved_return_ctx;
        self.env = saved_env;
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        step_result?;
        self.emit_dispatch_loop_body(
            span,
            contract,
            frame_layout,
            step_fn,
            dispatch_loop_fn,
            enclosing_return_ty,
        )?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

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
        let saved_effect_return_ctx = self.effect_function_return_context;
        let result = (|| -> Result<(), LlvmEmitError> {
            let entry_bb = self.context.append_basic_block(step_fn, "entry");
            self.builder.position_at_end(entry_bb);

            // Extract parameters.
            let state_ptr = step_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "step fn state param",
                    at: span.into(),
                })?
                .into_pointer_value();
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
                    _ => Some(self.builder.build_alloca(
                        self.llvm_basic_type_of(span, return_ty)?,
                        "step_function_return_val",
                    )?),
                };
                Some(EffectFunctionReturnContext {
                    return_bb,
                    return_alloca,
                    return_ty,
                })
            } else {
                None
            };
            self.effect_function_return_context = step_function_return_ctx;

            // Store resume values into frame.
            let resume_word_gep = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.resume_word_index(),
                "resume_word_ptr",
            )?;
            self.builder
                .build_store(resume_word_gep, resume_word_param)?;

            let resume_gc_ref_gep = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.resume_gc_ref_index(),
                "resume_gc_ref_ptr",
            )?;
            self.builder
                .build_store(resume_gc_ref_gep, resume_gc_ref_param)?;

            // Load state_tag for dispatch.
            let state_tag_gep = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                frame_layout.state_tag_index(),
                "state_tag_ptr",
            )?;
            let state_tag = self
                .builder
                .build_load(self.context.i32_type(), state_tag_gep, "state_tag")?
                .into_int_value();

            // Create basic blocks for each state.
            let states = contract.states();
            let unreachable_bb = self.context.append_basic_block(step_fn, "unreachable");

            let mut state_bb_map: HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>> =
                HashMap::new();
            for state in states {
                let label = format!("state_{}", state.id());
                let bb = self.context.append_basic_block(step_fn, &label);
                state_bb_map.insert(state.id(), bb);
            }

            // Build the switch dispatch.
            let i32_ty = self.context.i32_type();
            let cases: Vec<_> = states
                .iter()
                .map(|state| {
                    let tag = i32_ty.const_int(state.id() as u64, false);
                    let bb = state_bb_map[&state.id()];
                    (tag, bb)
                })
                .collect();
            self.builder
                .build_switch(state_tag, unreachable_bb, &cases)?;

            // Emit unreachable block.
            self.builder.position_at_end(unreachable_bb);
            self.builder.build_unreachable()?;

            // Emit ops and terminators for each state block.
            for state in states {
                let bb = state_bb_map[&state.id()];
                self.builder.position_at_end(bb);

                // Fresh env scope for this state's locals.
                self.env.push_scope();

                if matches!(state.context(), UnifiedStateContext::Cleanup { .. }) {
                    self.write_cleanup_flag(state_ptr, frame_layout, true, "cleanup_entered")?;
                }

                // Pre-populate frame slot locals so cross-state references work.
                // Each state gets its own GEP instructions (required for LLVM
                // SSA dominance: GEPs from sibling state BBs are not usable).
                self.populate_frame_slots_in_env(span, state_ptr, frame_layout, contract)?;

                let last_value =
                    self.emit_state_ops(span, state, state_ptr, frame_layout, contract)?;

                self.emit_state_terminator(
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

                self.env.pop_scope();
            }

            if let Some(effect_ctx) = step_function_return_ctx {
                self.builder.position_at_end(effect_ctx.return_bb);
                let return_value = self.load_effect_function_return_value(
                    span,
                    effect_ctx,
                    "step_function_return",
                )?;
                self.store_result_to_frame(span, return_value, state_ptr, frame_layout)?;
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    STATE_TAG_FUNCTION_RETURNED,
                    "state_tag_step_function_return",
                )?;
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            Ok(())
        })();
        self.effect_function_return_context = saved_effect_return_ctx;
        result
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
        let saved_effect_return_ctx = self.effect_function_return_context;
        let result = (|| -> Result<(), LlvmEmitError> {
            let entry_bb = self.context.append_basic_block(dispatch_loop_fn, "entry");
            self.builder.position_at_end(entry_bb);

            let frame_ptr = dispatch_loop_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch loop state param",
                    at: span.into(),
                })?
                .into_pointer_value();
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
                    _ => Some(self.builder.build_alloca(
                        self.llvm_basic_type_of(span, return_ty)?,
                        "dispatch_function_return_val",
                    )?),
                };
                Some(EffectFunctionReturnContext {
                    return_bb,
                    return_alloca,
                    return_ty,
                })
            } else {
                None
            };
            self.effect_function_return_context = dispatch_function_return_ctx;

            let i64_zero = self.context.i64_type().const_int(0, false);
            let gc_null = self.llvm_gc_i8_ptr_type().const_null();

            self.builder.build_call(
                step_fn,
                &[
                    frame_ptr.into(),
                    resume_word_param.into(),
                    resume_gc_ref_param.into(),
                ],
                "",
            )?;

            let dispatch_check_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_check");
            let dispatch_active_check_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_active_check");
            let dispatch_arm_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "dispatch_arm");
            let cleanup_entry_state = contract
                .cleanup_scopes()
                .first()
                .map(|scope| scope.entry_state());
            let handle_cleanup_propagate_check_bb = cleanup_entry_state.map(|_| {
                self.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_propagate_check")
            });
            let handle_cleanup_propagate_run_bb = cleanup_entry_state.map(|_| {
                self.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_propagate_run")
            });
            let handle_cleanup_done_check_bb = cleanup_entry_state.map(|_| {
                self.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_check")
            });
            let handle_cleanup_done_run_bb = cleanup_entry_state.map(|_| {
                self.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_run")
            });
            let handle_cleanup_done_complete_bb = cleanup_entry_state.map(|_| {
                self.context
                    .append_basic_block(dispatch_loop_fn, "handle_cleanup_done_complete")
            });
            let handle_propagate_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "handle_propagate");
            let handle_done_bb = self
                .context
                .append_basic_block(dispatch_loop_fn, "handle_done");
            let outward_target_bb =
                handle_cleanup_propagate_check_bb.unwrap_or(handle_propagate_bb);
            let arm_done_target_bb = handle_cleanup_done_check_bb.unwrap_or(handle_done_bb);

            self.builder.build_unconditional_branch(dispatch_check_bb)?;

            self.builder.position_at_end(dispatch_check_bb);
            // Terminal completion wins over any stale TLS active bit: once the
            // frame has reached a final state, the dispatch loop must continue
            // to the done/cleanup path instead of misclassifying it as an
            // outward-propagating perform.
            let dispatch_state_tag =
                self.read_state_tag(frame_ptr, frame_layout, "dispatch_state_tag")?;
            let dispatch_terminal = self.state_tag_matches_any(
                dispatch_state_tag,
                &[STATE_TAG_HANDLE_RETURNED, STATE_TAG_FUNCTION_RETURNED],
                "dispatch_terminal_state",
            )?;
            self.builder.build_conditional_branch(
                dispatch_terminal,
                arm_done_target_bb,
                dispatch_active_check_bb,
            )?;

            self.builder.position_at_end(dispatch_active_check_bb);
            let is_active = self.emit_effect_is_active_i1(span, "handle_dispatch_is_active")?;
            self.builder.build_conditional_branch(
                is_active,
                dispatch_arm_bb,
                arm_done_target_bb,
            )?;

            self.builder.position_at_end(dispatch_arm_bb);
            let read_op_tag_fn = self.declare_runtime_effect_perform_slot_read_op_tag();
            let op_tag_raw = self
                .builder
                .build_call(read_op_tag_fn, &[], "performed_op_tag")?
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform_slot_read_op_tag return",
                    at: span.into(),
                })?
                .into_int_value();
            let read_effect_instance_key_fn =
                self.declare_runtime_effect_perform_slot_read_effect_instance_key();
            let effect_instance_key_raw = self
                .builder
                .build_call(
                    read_effect_instance_key_fn,
                    &[],
                    "performed_effect_instance_key",
                )?
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform_slot_read_effect_instance_key return",
                    at: span.into(),
                })?
                .into_int_value();

            if contract.dispatch_entries().is_empty() {
                self.builder.build_unconditional_branch(outward_target_bb)?;
            } else {
                let unmatched_bb = self
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
                    let tag = self.effect_op_tag(op_fqn);
                    let tag_val = self.context.i32_type().const_int(tag as u64, false);
                    let dispatch_arms = dispatch_entry.arms();
                    if dispatch_arms.is_empty() {
                        continue;
                    }

                    let check_blocks = dispatch_arms
                        .iter()
                        .map(|dispatch_arm| {
                            self.context.append_basic_block(
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
                        let matching_keys = self.matching_effect_instance_keys_for_handled_effect(
                            unified_arm.effect_ty(),
                            op_fqn,
                        );
                        let arm_bb = self.emit_dispatch_arm_execution(
                            dispatch_loop_fn,
                            frame_ptr,
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

                        self.builder.position_at_end(check_blocks[index]);
                        if matching_keys.is_empty() {
                            self.builder.build_unconditional_branch(next_bb)?;
                        } else {
                            let arm_matches = self.int_matches_any_u32(
                                effect_instance_key_raw,
                                &matching_keys,
                                &format!("arm_{}_effect_instance_match", dispatch_arm.arm_id()),
                            )?;
                            self.builder
                                .build_conditional_branch(arm_matches, arm_bb, next_bb)?;
                        }
                    }
                }

                self.builder.position_at_end(dispatch_arm_bb);
                if dispatch_arm_bb.get_terminator().is_none() {
                    if cases.is_empty() {
                        self.builder.build_unconditional_branch(unmatched_bb)?;
                    } else {
                        self.builder
                            .build_switch(op_tag_raw, unmatched_bb, &cases)?;
                    }
                }

                self.builder.position_at_end(unmatched_bb);
                self.builder.build_unconditional_branch(outward_target_bb)?;
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
                self.builder.position_at_end(cleanup_propagate_check_bb);
                let cleanup_already_ran = self.read_cleanup_flag_i1(
                    frame_ptr,
                    frame_layout,
                    "cleanup_propagate_already_ran",
                )?;
                self.builder.build_conditional_branch(
                    cleanup_already_ran,
                    handle_propagate_bb,
                    cleanup_propagate_run_bb,
                )?;

                self.builder.position_at_end(cleanup_propagate_run_bb);
                self.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    cleanup_entry_state,
                    "set_cleanup_propagate_state",
                )?;
                let (cleanup_resume_word, cleanup_resume_gc_ref) = self.read_frame_resume_payload(
                    frame_ptr,
                    frame_layout,
                    "cleanup_propagate_resume_word",
                    "cleanup_propagate_resume_gc_ref",
                )?;
                self.builder.build_call(
                    step_fn,
                    &[
                        frame_ptr.into(),
                        cleanup_resume_word.into(),
                        cleanup_resume_gc_ref.into(),
                    ],
                    "",
                )?;
                self.builder
                    .build_unconditional_branch(handle_propagate_bb)?;

                self.builder.position_at_end(cleanup_done_check_bb);
                let cleanup_already_ran =
                    self.read_cleanup_flag_i1(frame_ptr, frame_layout, "cleanup_done_already_ran")?;
                self.builder.build_conditional_branch(
                    cleanup_already_ran,
                    cleanup_done_complete_bb,
                    cleanup_done_run_bb,
                )?;

                self.builder.position_at_end(cleanup_done_run_bb);
                self.capture_terminal_state_tag_for_cleanup(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_pre_state_tag",
                    "cleanup_done_completion_tag",
                )?;
                self.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    cleanup_entry_state,
                    "set_cleanup_done_state",
                )?;
                let (cleanup_resume_word, cleanup_resume_gc_ref) = self.read_frame_resume_payload(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_resume_word",
                    "cleanup_done_resume_gc_ref",
                )?;
                self.builder.build_call(
                    step_fn,
                    &[
                        frame_ptr.into(),
                        cleanup_resume_word.into(),
                        cleanup_resume_gc_ref.into(),
                    ],
                    "",
                )?;
                let cleanup_active =
                    self.emit_effect_is_active_i1(span, "cleanup_done_is_active")?;
                self.builder.build_conditional_branch(
                    cleanup_active,
                    handle_propagate_bb,
                    cleanup_done_check_bb,
                )?;

                self.builder.position_at_end(cleanup_done_complete_bb);
                self.restore_terminal_state_tag_after_cleanup(
                    frame_ptr,
                    frame_layout,
                    "cleanup_done_restore_terminal_state",
                )?;
                self.builder.build_unconditional_branch(handle_done_bb)?;
            }

            self.builder.position_at_end(handle_propagate_bb);
            self.builder.build_return(None)?;

            self.builder.position_at_end(handle_done_bb);
            let clear_fn = self.declare_runtime_effect_clear();
            self.builder.build_call(clear_fn, &[], "")?;
            self.builder.build_return(None)?;

            if let Some(effect_ctx) = dispatch_function_return_ctx {
                self.builder.position_at_end(effect_ctx.return_bb);
                let return_value = self.load_effect_function_return_value(
                    span,
                    effect_ctx,
                    "dispatch_function_return",
                )?;
                self.store_result_to_frame(span, return_value, frame_ptr, frame_layout)?;
                self.write_state_tag(
                    frame_ptr,
                    frame_layout,
                    STATE_TAG_FUNCTION_RETURNED,
                    "state_tag_dispatch_function_return",
                )?;
                self.builder
                    .build_unconditional_branch(arm_done_target_bb)?;
            }

            Ok(())
        })();
        self.effect_function_return_context = saved_effect_return_ctx;
        result
    }

    /// Pre-populate the env with GEP pointers for all frame user slots.
    ///
    /// Each state BB needs its own GEP instructions for LLVM SSA dominance.
    /// This ensures that cross-state local references (e.g. a local bound in
    /// state 0 but used in state 2's initializer) work correctly even if the
    /// plan builder didn't emit an explicit `ReadLocal` op.
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
            let llvm_index = frame_layout.user_slot_llvm_index(unified_slot.field_index());
            let slot_ptr = self.builder.build_struct_gep(
                frame_layout.frame_type,
                state_ptr,
                llvm_index,
                &format!("pre_slot_{}", id.as_u32()),
            )?;
            self.env.insert(
                id,
                CgLocal {
                    hir_ty: Some(type_id),
                    call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                    ty: cg_ty,
                    ptr: slot_ptr,
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
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let mut last_value: Option<CgValue<'ctx>> = None;

        for op in state.ops() {
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
                HandleStateOp::BindLocal { id, decl } => {
                    self.emit_bind_local_to_frame(
                        *id,
                        decl,
                        state_ptr,
                        frame_layout,
                        contract,
                        span,
                    )?;
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
                HandleStateOp::DeclareAnonymousVal { decl } => {
                    if let Some(init) = &decl.init {
                        let val = self.codegen_expr_in_expected_context(init, None)?;
                        last_value = Some(val);
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
                HandleStateOp::SuspendCall { expr, .. } => {
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
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
                HandleStateOp::ObjectInitAccessBoundary { expr, .. } => {
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
                    last_value = Some(val);
                }

                // --- Runtime raise boundary: evaluate expression ---
                HandleStateOp::RuntimeRaiseBoundary { expr, .. } => {
                    let val = self.codegen_expr_in_expected_context(expr, None)?;
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
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        span: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let target_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bind local type in state machine",
                at: decl.span.into(),
            })?;

        // Evaluate initializer.
        let init_val = match decl.init.as_ref() {
            Some(init_expr) => self.codegen_initializer_expr(init_expr, target_ty, decl.ty)?,
            None => self.default_value(span, target_ty)?,
        };

        // Find the frame slot for this local.
        let field_index = contract.frame().get_slot_field_index(id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "bind local: frame slot not found",
                at: decl.span.into(),
            },
        )?;
        let llvm_index = frame_layout.user_slot_llvm_index(field_index);

        // GEP into frame + store.
        let slot_ptr = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            llvm_index,
            &format!("frame_bind_{}", id.as_u32()),
        )?;
        self.store_local_value(decl.span, slot_ptr, target_ty, init_val)?;

        // Register in env so subsequent ops/exprs can reference this local.
        self.env.insert(
            id,
            CgLocal {
                hir_ty: Some(decl.ty),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(decl.ty)),
                ty: target_ty,
                ptr: slot_ptr,
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

        // GEP into frame.
        let slot_ptr = self.builder.build_struct_gep(
            frame_layout.frame_type,
            state_ptr,
            llvm_index,
            &format!("frame_read_{}", id.as_u32()),
        )?;

        // Register in env so subsequent ops can reference this local via
        // the standard `codegen_var_ref` → env lookup → load path.
        self.env.insert(
            id,
            CgLocal {
                hir_ty: Some(type_id),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                ty: cg_ty,
                ptr: slot_ptr,
                mutable: unified_slot.slot().mutable(),
            },
        );

        // Load and return.
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        let loaded = self.builder.build_load(llvm_ty, slot_ptr, "slot_val")?;
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
        self.store_local_value(at, slot_ptr, cg_ty, value)?;
        self.env.insert(
            resume_slot.id(),
            CgLocal {
                hir_ty: Some(resume_slot.ty()),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(resume_slot.ty())),
                ty: cg_ty,
                ptr: slot_ptr,
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

    fn emit_resume_after_call_site(
        &mut self,
        site_id: u32,
        source_span: crate::span::Span,
        resume_slot: &FrameSlot,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let step_fn = self.current_codegen_function(source_span)?;
        let current_callee_state = self.current_callee_suspend_state_ptr(
            source_span,
            &format!("site{site_id}_callee_suspend_state"),
        )?;
        let has_callee_state = self.ptr_is_non_null(
            source_span,
            current_callee_state,
            &format!("site{site_id}_has_callee_suspend_state"),
        )?;
        let replay_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_replay"));
        let inactive_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_inactive"));
        let merge_bb = self
            .context
            .append_basic_block(step_fn, &format!("site{site_id}_resume_merge"));
        self.builder
            .build_conditional_branch(has_callee_state, replay_bb, inactive_bb)?;

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
        self.emit_resume_payload_into_callee_suspend_state(
            source_span,
            current_callee_state,
            resume_word,
            resume_gc_ref,
        )?;
        let call_expr = self.lookup_suspend_call_expr(contract, site_id).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "resume.after.call source expression",
                at: source_span.into(),
            },
        )?;
        let call_result = self.codegen_expr_in_expected_context(call_expr, None)?;
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
        frame_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<(), LlvmEmitError> {
        for unified_slot in contract.frame().slots() {
            let slot = unified_slot.slot();
            if slot.owner_arm().is_some() || !slot.seed_from_outer_scope() {
                continue;
            }
            let local = self
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
                    let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        local.ptr,
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
            let storage_ptr = self.builder.build_pointer_cast(
                storage_ptr,
                self.llvm_ptr_type(AddressSpace::default()),
                &format!("writeback_outer_slot_target_{}", slot.id().as_u32()),
            )?;
            self.store_local_value(at, storage_ptr, slot_cg_ty, value)?;
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
                    let is_active =
                        self.emit_effect_is_active_i1(span, &format!("site{site_id}_is_active"))?;

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

                // Record the body resume state on the continuation itself.
                // Handler dispatch reuses frame.state_tag for arm execution,
                // so the continuation cannot rely on the mutable frame field
                // remaining pointed at the suspended body state.
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

                // Lift any currently outstanding ordinary indirect-callee
                // suspend state into the continuation itself. From this point
                // on the continuation, not thread-local TLS, is the
                // authoritative owner across handle exit / cross-thread resume.
                let get_callee_state = self.declare_runtime_callee_suspend_state_get();
                let captured_callee_state = self
                    .builder
                    .build_call(get_callee_state, &[], "captured_callee_suspend_state")?
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "callee_suspend_state_get return value",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let cont_callee_state_gep = self.builder.build_struct_gep(
                    cont_ty,
                    cont,
                    8, // captured_callee_suspend_state
                    "cont_callee_suspend_state",
                )?;
                self.builder
                    .build_store(cont_callee_state_gep, captured_callee_state)?;

                let clear_callee_state = self.declare_runtime_callee_suspend_state_clear();
                self.builder
                    .build_call(clear_callee_state, &[], "clear_callee_suspend_state")?;
                let unpin_callee_state = self.declare_runtime_gc_unpin();
                self.builder.build_call(
                    unpin_callee_state,
                    &[captured_callee_state.into()],
                    "unpin_callee_suspend_state_after_capture",
                )?;

                // Store the continuation pointer into the dedicated runtime
                // slot so later step_fn re-entry cannot overwrite it by
                // refreshing resume_gc_ref from the call parameters.
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "frame_continuation_ptr",
                )?;
                self.builder.build_store(cont_gep, cont)?;

                // Set the TLS active flag to signal that an effect was
                // performed.
                let set_active = self.declare_runtime_effect_set_active();
                self.builder.build_call(set_active, &[], "")?;

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
                // ImmediateResume arm: the arm has computed a resume value
                // (in last_value).  Write it into the continuation's
                // resume_word/resume_gc_ref and call continuation_resume.
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

                // Write the resume payload into the continuation struct.
                if let Some(val) = last_value {
                    self.write_resume_payload_to_continuation(span, val, cont_ptr)?;
                }

                // Call scoop_continuation_resume(k).
                let resume_fn = self.declare_runtime_continuation_resume();
                self.builder.build_call(resume_fn, &[cont_ptr.into()], "")?;

                // After resume returns, the step_fn has finished (or
                // suspended again — the dispatch loop handles that).
                // Return void to let the dispatch loop continue.
                self.write_back_outer_scope_frame_slots(span, state_ptr, frame_layout, contract)?;
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::ArmMaterializeContinuation => {
                // EscapeContinuation arm: the continuation has already been
                // bound as a local (in ExecuteArmBody).  The arm body calls
                // k.resume() at its discretion.  Just store any result and
                // mark handle as returned.
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

    /// Read the TLS effect active flag and coerce it to an LLVM `i1`.
    fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let is_active_fn = self.declare_runtime_effect_is_active();
        let active_raw = self
            .builder
            .build_call(is_active_fn, &[], name)?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return",
                at: at.into(),
            })?
            .into_int_value();
        Ok(self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            active_raw,
            self.context.i32_type().const_int(0, false),
            &format!("{name}_bool"),
        )?)
    }

    fn suspend_site_uses_inactive_continue_path(kind: &SuspendSiteKind) -> bool {
        matches!(
            kind,
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::RuntimeRaise { .. }
                | SuspendSiteKind::ObjectInitAccess { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::NestedHandleBoundary { .. }
        )
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
        self.builder.build_store(gc_ref_gep, gc_ref)?;
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
        self.effect_function_return_context
            .map(|ctx| ctx.return_ty)
            .or(self.current_fun_return_ty)
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

        if let Some(effect_ctx) = self.effect_function_return_context {
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

        self.seed_outer_scope_frame_slots(span, frame_ptr, &frame_layout, &contract)?;

        // Set state_tag to entry state.
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
            _ => {
                let llvm_ty = self.llvm_basic_type_of(span, result_cg_ty)?;
                Some(self.builder.build_alloca(llvm_ty, "handle_result_slot")?)
            }
        };
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

        self.write_back_outer_scope_frame_slots(span, frame_ptr, &frame_layout, &contract)?;

        if self.ordinary_effect_propagation_enabled() {
            self.emit_ordinary_non_resuming_effect_exit(span, "handle_outward_effect")?;
        }

        if let Some(result_slot) = result_slot {
            let default = self.default_value(span, result_cg_ty)?;
            self.store_local_value(span, result_slot, result_cg_ty, default)?;
        }
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
        let return_value =
            self.read_result_from_frame(span, declared_return_cg, frame_ptr, &frame_layout)?;
        self.finish_enclosing_function_return_path(span, declared_return_cg, return_value)?;

        self.builder.position_at_end(handle_complete_bb);

        if let Some(result_slot) = result_slot {
            let result =
                self.read_result_from_frame(span, result_cg_ty, frame_ptr, &frame_layout)?;
            self.store_local_value(span, result_slot, result_cg_ty, result)?;
        }
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
        frame_ptr: PointerValue<'ctx>,
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

        let clear_active_fn = self.declare_runtime_effect_clear_active();
        self.builder.build_call(clear_active_fn, &[], "")?;

        self.write_state_tag(
            frame_ptr,
            frame_layout,
            unified_arm.entry_state(),
            &format!("set_arm_state_{arm_id}"),
        )?;

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

        let payload_expr = match &expr.kind {
            hir::ExprKind::Perform { args, .. } => match args.as_slice() {
                [] => None,
                [hir::CallArg::Positional(payload)] => Some(payload),
                [hir::CallArg::Named { value, .. }] => Some(value),
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "state machine perform arity",
                        at: expr.span.into(),
                    });
                }
            },
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "state machine perform payload expr",
                    at: expr.span.into(),
                });
            }
        };

        // Evaluate only the payload expression. Re-emitting the entire
        // `perform` expression would recurse into `codegen_perform_expr`,
        // which overwrites the TLS perform slot with the default resume
        // placeholder for generic callers.
        let payload_val = if let Some(payload_expr) = payload_expr {
            self.codegen_expr_in_expected_context(payload_expr, None)?
        } else {
            CgValue::unit()
        };

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
        _span: crate::span::Span,
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

        // Set up binder locals: read from perform slot and store to frame slots.
        for binder in &arm.op.binders {
            let binder_cg_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "arm binder type",
                        at: binder.span.into(),
                    })?;

            // Read the binder value from the TLS perform slot.
            let binder_val = self.read_binder_from_perform_slot(binder.span, binder_cg_ty)?;

            // If there's a frame slot for this binder, store to frame.
            if let Some(field_index) = contract.frame().get_slot_field_index(binder.id) {
                let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                let slot_ptr = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    llvm_index,
                    &format!("arm_binder_{}", binder.id.as_u32()),
                )?;
                self.store_local_value(binder.span, slot_ptr, binder_cg_ty, binder_val)?;
                self.env.insert(
                    binder.id,
                    CgLocal {
                        hir_ty: Some(binder.ty),
                        call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(binder.ty)),
                        ty: binder_cg_ty,
                        ptr: slot_ptr,
                        mutable: false,
                    },
                );
            } else {
                // No frame slot — allocate stack local.
                let llvm_ty = self.llvm_basic_type_of(binder.span, binder_cg_ty)?;
                let alloca = self
                    .builder
                    .build_alloca(llvm_ty, &format!("binder_{}", binder.name))?;
                self.store_local_value(binder.span, alloca, binder_cg_ty, binder_val)?;
                self.env.insert(
                    binder.id,
                    CgLocal {
                        hir_ty: Some(binder.ty),
                        call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(binder.ty)),
                        ty: binder_cg_ty,
                        ptr: alloca,
                        mutable: false,
                    },
                );
            }
        }

        // EscapeContinuation arms bind the continuation as a local.  Immediate
        // resume arms are handled by dedicated tail-expression lowering below,
        // so they no longer need a placeholder `resume` local.
        match arm.kind {
            hir::HandleArmKind::ImmediateResume { .. } => {}
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                // Load the continuation pointer from the dedicated runtime
                // continuation slot (where Suspend stored it).
                let cont_gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.continuation_index(),
                    "load_continuation",
                )?;
                let cont_ptr = self
                    .builder
                    .build_load(self.llvm_gc_i8_ptr_type(), cont_gep, "continuation_val")?
                    .into_pointer_value();

                // Find or alloc frame slot for the continuation local.
                if let Some(field_index) = contract.frame().get_slot_field_index(continuation) {
                    let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                    let slot_ptr = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        llvm_index,
                        "cont_slot",
                    )?;
                    self.builder.build_store(slot_ptr, cont_ptr)?;
                    self.env.insert(
                        continuation,
                        CgLocal {
                            hir_ty: None,
                            call_may_suspend: false,
                            ty: CgTy::Ref,
                            ptr: slot_ptr,
                            mutable: false,
                        },
                    );
                } else {
                    let alloca = self
                        .builder
                        .build_alloca(self.llvm_gc_i8_ptr_type(), "cont_local")?;
                    self.builder.build_store(alloca, cont_ptr)?;
                    self.env.insert(
                        continuation,
                        CgLocal {
                            hir_ty: None,
                            call_may_suspend: false,
                            ty: CgTy::Ref,
                            ptr: alloca,
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
            if self.env.get(local_id).is_some() {
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
                    let llvm_index = frame_layout.user_slot_llvm_index(field_index);
                    let slot_ptr = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        llvm_index,
                        &format!("capture_{}", local_id.as_u32()),
                    )?;
                    self.env.insert(
                        local_id,
                        CgLocal {
                            hir_ty: Some(type_id),
                            call_may_suspend: self
                                .local_call_may_suspend_from_hir_ty(Some(type_id)),
                            ty: cg_ty,
                            ptr: slot_ptr,
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
        // rest of the arm body. Setting `current_fun_return_ty = Never` lets
        // the existing ordinary propagation helpers terminate the step
        // function immediately when arm-local code performs.
        let saved_return_ctx = self.return_context.take();
        let saved_return_ty = self.current_fun_return_ty.take();
        self.current_fun_return_ty = Some(CgTy::Never);

        let result = match arm.kind {
            hir::HandleArmKind::ImmediateResume { resume } => {
                let rewritten = rewrite_immediate_resume_arm_body(arm, resume)?;
                let payload_cg_ty =
                    self.cg_ty_of(rewritten.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "immediate resume payload type",
                            at: rewritten.span.into(),
                        })?;
                self.codegen_expr_in_expected_context(&rewritten, Some(payload_cg_ty))
            }
            _ => self.codegen_expr_in_expected_context(&arm.body, None),
        };

        self.current_fun_return_ty = saved_return_ty;
        self.return_context = saved_return_ctx;
        result
    }

    /// Read a binder value from the TLS perform slot.
    fn read_binder_from_perform_slot(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let read_word_fn = self.declare_runtime_effect_perform_slot_read_u64();
        let word = self
            .builder
            .build_call(read_word_fn, &[], "binder_word")?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_u64 return",
                at: at.into(),
            })?
            .into_int_value();
        let read_gc_ref_fn = self.declare_runtime_effect_perform_slot_read_gc_ref();
        let gc_ref = self
            .builder
            .build_call(read_gc_ref_fn, &[], "binder_gc_ref")?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_gc_ref return",
                at: at.into(),
            })?
            .into_pointer_value();
        self.decode_effect_transport_value(at, word, gc_ref, cg_ty)
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

    use crate::llvm::{emit_minimal_main_ir, emit_minimal_main_ir_from_lowered_hir};
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::{SourceFile, SourceMap};
    use crate::ty::TypeStore;
    use crate::typecheck;

    #[test]
    fn immediate_resume_arm_body_rewrites_tail_resume_call_to_payload_expr() {
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
        Yield.next() -> resume {
            println("in_handler")
            resume(41)
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::ImmediateResume { resume } = arm.kind else {
            panic!("expected immediate-resume arm");
        };

        let rewritten = rewrite_immediate_resume_arm_body(arm, resume)
            .expect("immediate-resume arm should rewrite");
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
    fn immediate_resume_arm_body_rewrites_if_branch_tails() {
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
        Yield.next() -> resume {
            if (flag) {
                resume(1)
            } else {
                resume(2)
            }
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::ImmediateResume { resume } = arm.kind else {
            panic!("expected immediate-resume arm");
        };

        let rewritten = rewrite_immediate_resume_arm_body(arm, resume)
            .expect("immediate-resume arm should rewrite");
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
    fn immediate_resume_arm_body_rewrites_non_block_tail_resume_call() {
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
        Yield.next() -> resume {
            println("in_handler")
            resume(41)
        }
    }
}
"#,
        );

        let (_, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let arm = handle.arms.first().expect("expected an arm");
        let hir::HandleArmKind::ImmediateResume { resume } = arm.kind else {
            panic!("expected immediate-resume arm");
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
        let rewritten = rewrite_immediate_resume_arm_body(&direct_arm, resume)
            .expect("non-block immediate-resume arm should rewrite");

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
    var saved: Continuation<Unit>? = None()
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
    fn suspend_ir_captures_callee_suspend_state_into_continuation() {
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
            ir.contains("@scoop_callee_suspend_state_get"),
            "suspend path should read any outstanding callee suspend state from runtime TLS"
        );
        assert!(
            ir.contains("@scoop_callee_suspend_state_clear"),
            "suspend path should clear the runtime TLS callee suspend state after capture"
        );
        assert!(
            ir.contains("cont_callee_suspend_state"),
            "continuation layout should materialize a dedicated field GEP for captured callee suspend state"
        );
        assert!(
            ir.contains("captured_callee_suspend_state"),
            "IR should name the captured callee suspend state value flowing into the continuation"
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

        assert!(
            ir.contains("site0_is_active"),
            "outer handle should still check TLS active around the indirect if-branch callee call"
        );
        assert!(
            ir.contains("site0_active"),
            "outer handle should preserve the active-dispatch path for the indirect if-branch callee call"
        );
        assert!(
            ir.contains("callee_suspend_entry_is_resume"),
            "ordinary if-branch callee should still build a fresh/resume dual-entry contract"
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
