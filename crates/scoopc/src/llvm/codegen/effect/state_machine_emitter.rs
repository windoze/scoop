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

use std::collections::HashMap;

use super::unified_state_machine_skeleton::FrameSlot;
use super::*;

use super::unified_state_machine_skeleton::{
    HandleBranchCondition, HandleStateOp, UnifiedFrameField, UnifiedFrameSystemField,
    UnifiedHandleLoweringContract, UnifiedState, UnifiedStateTerminator,
};

/// System field indices in the frame struct.
///
/// Layout:
///   field 0: header      (ScoopGcObjectHeader)
///   field 1: state_tag   (i32)   — current state / PC
///   field 2: resume_word (i64)   — scalar resume payload / handle result word
///   field 3: resume_gc_ref (ptr addrspace(1)) — GC ref resume payload / handle result ref
///   field 4+: optional system fields (cleanup_flag, one_shot_flag)
///   then: user slots
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
    continuation_index: u32,
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
    ///   and optionally cleanup_flag (i32), one_shot_flag (i32).
    /// - User slots: one field per `UnifiedFrameSlot`, typed according to the
    ///   slot's `TypeId`.
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

        // Build the system field types in declaration order.
        let mut field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = vec![
            header_ty.into(), // GC object header
            i32_ty.into(),    // state_tag
            i64_ty.into(),    // resume_word
            gc_ptr_ty.into(), // resume_gc_ref
        ];

        // Optional system fields from the schema.
        for field in system_fields {
            match field {
                UnifiedFrameField::System(UnifiedFrameSystemField::StateTag)
                | UnifiedFrameField::System(UnifiedFrameSystemField::ResumeWord)
                | UnifiedFrameField::System(UnifiedFrameSystemField::ResumeGcRef) => {
                    // Already added above.
                }
                UnifiedFrameField::System(UnifiedFrameSystemField::CleanupFlag) => {
                    field_types.push(i32_ty.into());
                }
                UnifiedFrameField::System(UnifiedFrameSystemField::OneShotFlag) => {
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
            continuation_index,
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

    /// Generate the step function for a state machine.
    ///
    /// The step function has the continuation step signature:
    ///   `void step_fn(ptr addrspace(1) %state, i64 %resume_word, ptr addrspace(1) %resume_gc_ref)`
    ///
    /// On entry it stores resume_word and resume_gc_ref into the frame, loads
    /// state_tag, and dispatches via `switch` to per-state basic blocks.
    ///
    /// Each state block emits its ops and terminates with the
    /// appropriate LLVM terminator (branch, return, etc.).
    fn emit_effect_step_function(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, LlvmEmitError> {
        // Step function signature: (ptr addrspace(1), i64, ptr addrspace(1)) -> void
        let state_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();

        let param_tys: [inkwell::types::BasicMetadataTypeEnum<'ctx>; 3] =
            [state_ptr_ty.into(), i64_ty.into(), gc_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);

        let fn_name = format!("scoop.effect.step.{:x}", span.start ^ (span.end << 16));
        let step_fn = self.module.add_function(&fn_name, fn_ty, None);
        step_fn.set_call_conventions(0);

        // Save caller's codegen context.
        let saved_block = self.builder.get_insert_block();
        let saved_env = std::mem::take(&mut self.env);
        let saved_return_ctx = self.return_context.take();
        let saved_return_ty = self.current_fun_return_ty.take();
        let saved_loop_stack = std::mem::take(&mut self.loop_context_stack);

        // --- Generate step function body ---
        let result = self.emit_step_function_body(span, contract, frame_layout, step_fn);

        // Restore caller's codegen context.
        self.loop_context_stack = saved_loop_stack;
        self.current_fun_return_ty = saved_return_ty;
        self.return_context = saved_return_ctx;
        self.env = saved_env;
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        result?;
        Ok(step_fn)
    }

    /// Inner body of step function generation, separated so we can use `?`
    /// freely while the caller handles save/restore.
    fn emit_step_function_body(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
        step_fn: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
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

        let mut state_bb_map: HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>> = HashMap::new();
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

            // Pre-populate frame slot locals so cross-state references work.
            // Each state gets its own GEP instructions (required for LLVM
            // SSA dominance: GEPs from sibling state BBs are not usable).
            self.populate_frame_slots_in_env(span, state_ptr, frame_layout, contract)?;

            let last_value = self.emit_state_ops(span, state, state_ptr, frame_layout, contract)?;

            self.emit_state_terminator(
                span,
                state.terminator(),
                last_value,
                state_ptr,
                frame_layout,
                contract,
                &state_bb_map,
                step_fn,
            )?;

            self.env.pop_scope();
        }

        Ok(())
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
                // VarRef is handled separately: the plan builder decomposes
                // composite expressions (Call, Binary, etc.) by recursing
                // into sub-expressions, emitting a VarRef op for each callee
                // or operand.  These standalone VarRef results are always
                // overwritten by the subsequent composite op (which carries
                // the full original expression and re-evaluates everything).
                // For top-level function names, `codegen_var_ref` fails
                // because functions are not standalone values.  We produce a
                // unit fallback for any VarRef codegen failure since the
                // result is never consumed.
                HandleStateOp::VarRef { expr } => {
                    let val = self
                        .codegen_expr_in_expected_context(expr, None)
                        .unwrap_or(CgValue::unit());
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
                            let val = self.codegen_expr_in_expected_context(val_expr, None)?;
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
                        self.emit_resume_value_to_frame_slot(
                            *source_span,
                            resume_slot,
                            state_ptr,
                            frame_layout,
                            contract,
                        )?;
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

    fn emit_resume_value_to_frame_slot(
        &mut self,
        at: crate::span::Span,
        resume_slot: &FrameSlot,
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
        let resume_value = self.read_result_from_frame(at, cg_ty, state_ptr, frame_layout)?;
        self.store_local_value(at, slot_ptr, cg_ty, resume_value)?;
        Ok(())
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
            if slot.owner_arm().is_some() {
                continue;
            }
            let Some(local) = self.env.get(slot.id()) else {
                continue;
            };

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
        terminator: &UnifiedStateTerminator,
        last_value: Option<CgValue<'ctx>>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        contract: &UnifiedHandleLoweringContract,
        state_bb_map: &HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>>,
        step_fn: inkwell::values::FunctionValue<'ctx>,
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
                if self.state_is_handle_result_exit(contract, *next_state)
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
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::Suspend { resume_state, .. } => {
                // Save the resume state_tag so step_fn re-enters at the
                // right state after the handler arm resumes.
                self.write_state_tag(
                    state_ptr,
                    frame_layout,
                    *resume_state,
                    "state_tag_suspend_resume",
                )?;

                // Allocate a continuation object (GC-managed) that captures
                // the frame pointer and step_fn.
                let step_fn_ptr = step_fn.as_global_value().as_pointer_value();
                let cont_alloc = self.declare_runtime_continuation_alloc();
                let cont = self
                    .builder
                    .build_call(
                        cont_alloc,
                        &[state_ptr.into(), step_fn_ptr.into()],
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
                self.builder.build_return(None)?;
            }

            UnifiedStateTerminator::CleanupEnter { next_state, .. } => {
                if let Some(val) = last_value {
                    self.store_result_to_frame(span, val, state_ptr, frame_layout)?;
                }
                // Branch unconditionally to the cleanup scope's entry state.
                // The cleanup states (finally block) are part of the same step
                // function's state table and will execute their ops normally,
                // eventually flowing back through Goto → ReturnHandle.
                let target_bb = self.lookup_state_bb(*next_state, state_bb_map, span)?;
                self.builder.build_unconditional_branch(target_bb)?;
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
                self.builder.build_return(None)?;
            }
        }

        Ok(())
    }

    fn state_is_handle_result_exit(
        &self,
        contract: &UnifiedHandleLoweringContract,
        state_id: u32,
    ) -> bool {
        let Some(state) = contract.machine().get_state(state_id) else {
            return false;
        };
        matches!(state.terminator(), UnifiedStateTerminator::ReturnHandle)
            && state
                .ops()
                .iter()
                .all(|op| matches!(op, HandleStateOp::ReturnToEnclosingExpression))
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
        match val.ty {
            CgTy::Unit | CgTy::Never => {
                // No runtime value to store.
            }
            CgTy::String | CgTy::Ref => {
                // GC-managed pointer → resume_gc_ref.
                if let Some(raw) = val.value {
                    let gep = self.builder.build_struct_gep(
                        frame_layout.frame_type,
                        state_ptr,
                        frame_layout.resume_gc_ref_index(),
                        "result_gc_ref",
                    )?;
                    self.builder.build_store(gep, raw)?;
                }
            }
            _ => {
                // Scalar / composite → coerce to u64 word and store.
                let word = self.coerce_u64_word(span, val)?;
                let gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    state_ptr,
                    frame_layout.resume_word_index(),
                    "result_word",
                )?;
                self.builder.build_store(gep, word)?;
            }
        }
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
        match result_cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::String | CgTy::Ref => {
                // Load GC ref from resume_gc_ref.
                let gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    frame_ptr,
                    frame_layout.resume_gc_ref_index(),
                    "read_result_gc_ref",
                )?;
                let gc_ptr_ty = self.llvm_gc_i8_ptr_type();
                let loaded = self.builder.build_load(gc_ptr_ty, gep, "result_ref")?;
                self.cg_value_from_loaded(span, result_cg_ty, loaded)
            }
            _ => {
                // Load scalar word from resume_word and narrow to target type.
                let gep = self.builder.build_struct_gep(
                    frame_layout.frame_type,
                    frame_ptr,
                    frame_layout.resume_word_index(),
                    "read_result_word",
                )?;
                let i64_ty = self.context.i64_type();
                let loaded = self
                    .builder
                    .build_load(i64_ty, gep, "result_u64")?
                    .into_int_value();

                // Narrow from u64 to the actual result type.
                self.narrow_u64_word_to_cg_value(span, loaded, result_cg_ty)
            }
        }
    }

    /// Convert a u64 word (loaded from resume_word) back to the target CgTy.
    fn narrow_u64_word_to_cg_value(
        &mut self,
        span: crate::span::Span,
        word: IntValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match target {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool => {
                let trunc = self.builder.build_int_truncate(
                    word,
                    self.context.bool_type(),
                    "u64_to_bool",
                )?;
                Ok(CgValue::bool(trunc))
            }
            CgTy::Int(int_ty) => {
                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let narrowed = self.cast_int(word, from, int_ty)?;
                Ok(CgValue::int(narrowed, int_ty))
            }
            CgTy::Float64 => {
                let f64_ty = self.context.f64_type();
                let bits = self
                    .builder
                    .build_bit_cast(word, f64_ty, "u64_to_f64")?
                    .into_float_value();
                Ok(CgValue::float(bits, CgTy::Float64))
            }
            CgTy::Float32 => {
                let i32_trunc =
                    self.builder
                        .build_int_truncate(word, self.context.i32_type(), "u64_to_u32")?;
                let f32_ty = self.context.f32_type();
                let bits = self
                    .builder
                    .build_bit_cast(i32_trunc, f32_ty, "u32_to_f32")?
                    .into_float_value();
                Ok(CgValue::float(bits, CgTy::Float32))
            }
            CgTy::String | CgTy::Ref => {
                // Should not reach here — GC refs use resume_gc_ref.
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "narrow u64 to gc ref",
                    at: span.into(),
                })
            }
            CgTy::Enum(enum_ty) => {
                let layout = self.cg_enum_layout(span, enum_ty)?;
                match layout.repr {
                    CgEnumRepr::ValueOnly { underlying } => {
                        // Value-only enums are plain integers — truncate u64.
                        let narrowed = self.cast_int(
                            word,
                            IntTy {
                                bits: 64,
                                signed: false,
                            },
                            underlying,
                        )?;
                        Ok(CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(narrowed.into()),
                        })
                    }
                    CgEnumRepr::TaggedUnion => {
                        // Construct { tag, payload_word=0, payload_ptr=null }
                        // from the u64 word (which encodes the tag).
                        // Only valid for fieldless-variant enums transported
                        // via the perform slot (e.g. RuntimeError).
                        let llvm_ty = self.llvm_enum_value_type(span, enum_ty)?.into_struct_type();
                        let tag_i32 = self.builder.build_int_truncate(
                            word,
                            self.context.i32_type(),
                            "enum_tag_from_u64",
                        )?;
                        let payload_word_ty = self.int_type(self.enum_payload_ty());
                        let payload_ptr_ty = self.llvm_gc_i8_ptr_type();

                        let mut agg: AggregateValueEnum<'_> = llvm_ty.get_undef().into();
                        agg = self
                            .builder
                            .build_insert_value(agg, tag_i32, 0, "enum_tag")?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_ty.const_int(0, false),
                            1,
                            "enum_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_ty.const_null(),
                            2,
                            "enum_payload_ptr",
                        )?;
                        Ok(CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(agg.as_basic_value_enum()),
                        })
                    }
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "narrow u64 to niche enum (not yet supported)",
                        at: span.into(),
                    }),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "narrow u64 to composite type (not yet supported)",
                at: span.into(),
            }),
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

        // 2. Generate frame layout and step function.
        let frame_layout = self.emit_effect_frame_layout(span, &contract)?;
        let step_fn = self.emit_effect_step_function(span, &contract, &frame_layout)?;

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

        // Allocate a handler frame on the stack for handler-stack registration.
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self
            .builder
            .build_alloca(handler_frame_ty, "handler_frame")?;

        // Zero-initialize the handler frame.
        let handler_frame_size = self.target_data.get_store_size(&handler_frame_ty);
        let handler_frame_size_val = self
            .llvm_ptr_sized_int_type(None)
            .const_int(handler_frame_size, false);
        let handler_frame_i8 = self.builder.build_pointer_cast(
            handler_frame_ptr,
            self.llvm_i8_ptr_type(),
            "handler_frame_i8",
        )?;
        let zero = self.context.i8_type().const_zero();
        let _ = self
            .builder
            .build_memset(handler_frame_i8, 1, zero, handler_frame_size_val)?;

        // Push the handler frame onto the handler stack for each effect
        // operation handled by this handler.  For simplicity, we push once
        // with the first dispatch entry's op_tag; a more complete
        // implementation would push one frame per op.
        // For now, if there are dispatch entries, push for the first one.
        let has_dispatch = !contract.dispatch_entries().is_empty();
        if has_dispatch {
            // Use op_tag 0 as a catch-all for the handler.
            // Individual op_tags are checked in the dispatch logic.
            let first_op_fqn = contract.dispatch_entries()[0].op_fqn();
            let op_tag = self.effect_op_tag(first_op_fqn);
            let op_tag_val = self.context.i32_type().const_int(op_tag as u64, false);
            let push_fn = self.declare_runtime_effect_handler_stack_push();
            self.builder
                .build_call(push_fn, &[handler_frame_ptr.into(), op_tag_val.into()], "")?;
        }

        // 5. Call the step function for initial body execution.
        let i64_zero = self.context.i64_type().const_int(0, false);
        let gc_null = self.llvm_gc_i8_ptr_type().const_null();
        self.builder.build_call(
            step_fn,
            &[frame_ptr.into(), i64_zero.into(), gc_null.into()],
            "",
        )?;

        // 6. Dispatch loop.
        //
        // After each step_fn return, check the TLS active flag.
        // If active → a perform happened; read op_tag, dispatch to arm.
        // If not active → body completed (or early return); exit loop.
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

        let dispatch_check_bb = self
            .context
            .append_basic_block(current_fn, "dispatch_check");
        let dispatch_arm_bb = self.context.append_basic_block(current_fn, "dispatch_arm");
        let handle_done_bb = self.context.append_basic_block(current_fn, "handle_done");

        // Jump to the dispatch check after the initial step_fn call.
        self.builder.build_unconditional_branch(dispatch_check_bb)?;

        // --- dispatch_check: test active flag ---
        self.builder.position_at_end(dispatch_check_bb);
        let is_active_fn = self.declare_runtime_effect_is_active();
        let active_raw = self
            .builder
            .build_call(is_active_fn, &[], "is_active")?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return",
                at: span.into(),
            })?
            .into_int_value();
        let is_active = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            active_raw,
            self.context.i32_type().const_int(0, false),
            "active_bool",
        )?;
        self.builder
            .build_conditional_branch(is_active, dispatch_arm_bb, handle_done_bb)?;

        // --- dispatch_arm: read op_tag, clear active, dispatch to arm ---
        self.builder.position_at_end(dispatch_arm_bb);

        // Read op_tag from TLS perform slot.
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

        // Clear only the active flag. The arm body still needs the perform
        // slot payload for binder setup; the full slot reset happens once the
        // handle expression finishes.
        let clear_fn = self.declare_runtime_effect_clear_active();
        self.builder.build_call(clear_fn, &[], "")?;

        // Build a switch on op_tag → arm entry state.
        // For each dispatch entry, compute the op_tag and route to the
        // arm's entry state in the step function.  The arm states are
        // inside the step_fn, so we call step_fn with the arm's entry
        // state pre-set in state_tag.
        //
        // Since the arm states live inside the step function (not the
        // caller), we call step_fn again after setting state_tag to the
        // arm's entry state.
        if contract.dispatch_entries().is_empty() {
            // No arms to dispatch to — just branch to done.
            self.builder.build_unconditional_branch(handle_done_bb)?;
        } else {
            // For each dispatch entry, create a handler arm block.
            let unmatched_bb = self
                .context
                .append_basic_block(current_fn, "dispatch_unmatched");

            let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();

            for dispatch_entry in contract.dispatch_entries() {
                let op_fqn = dispatch_entry.op_fqn();
                let tag = self.effect_op_tag(op_fqn);
                let tag_val = self.context.i32_type().const_int(tag as u64, false);

                // For each arm in this dispatch entry, pick the first arm
                // (single-arm dispatch is the common case for now).
                if let Some(first_arm) = dispatch_entry.arms().first() {
                    let arm_bb = self
                        .context
                        .append_basic_block(current_fn, &format!("arm_{}", first_arm.arm_id()));
                    cases.push((tag_val, arm_bb));

                    // In the arm block: set state_tag to arm entry state,
                    // call step_fn, then loop back to dispatch_check.
                    self.builder.position_at_end(arm_bb);

                    let arm_entry_state = first_arm.entry_state();
                    self.write_state_tag(
                        frame_ptr,
                        &frame_layout,
                        arm_entry_state,
                        &format!("set_arm_state_{}", first_arm.arm_id()),
                    )?;

                    // The arm's binder values come from the perform slot,
                    // but the arm body reads them via frame slots (set up
                    // by ExecuteArmBody ops inside the step_fn).
                    // Just call step_fn and let the internal arm states
                    // handle binder setup and body execution.
                    self.builder.build_call(
                        step_fn,
                        &[frame_ptr.into(), i64_zero.into(), gc_null.into()],
                        "",
                    )?;

                    // After arm execution, loop back to check if more
                    // performs happened (the arm may have resumed the
                    // body which then performed again).
                    self.builder.build_unconditional_branch(dispatch_check_bb)?;
                }
            }

            // Position back at dispatch_arm to emit the switch.
            // We need to re-position because we moved the builder to arm blocks.
            // Actually, the switch should be built at the end of dispatch_arm_bb
            // before the arm blocks.  Let me restructure: build arm blocks first
            // (without switch), then go back and emit switch at dispatch_arm_bb.
            //
            // The dispatch_arm_bb currently has: read_op_tag + clear.
            // We need to append the switch after those.  But we already moved
            // to other blocks.  The switch needs to be the terminator of
            // dispatch_arm_bb.  Since we built the arm blocks after positioning
            // at dispatch_arm_bb's ops, we need to go back.

            // Actually, we already read op_tag and cleared — the dispatch_arm_bb
            // doesn't have a terminator yet because we moved to arm blocks.
            // However, inkwell appends instructions to whatever block is current.
            // The op_tag read and clear are in dispatch_arm_bb.  The arm block
            // code was positioned in separate blocks.  So dispatch_arm_bb still
            // needs its terminator.

            self.builder.position_at_end(dispatch_arm_bb);
            // Note: the read_op_tag and clear calls are already in this block
            // from the code above.  However, because we moved the builder to
            // arm blocks and came back, the instructions should still be
            // correctly placed.  Let's verify by checking if there's a terminator.
            if dispatch_arm_bb.get_terminator().is_none() {
                self.builder
                    .build_switch(op_tag_raw, unmatched_bb, &cases)?;
            }

            // Unmatched op_tag: this shouldn't normally happen for well-formed
            // programs.  Branch to handle_done.
            self.builder.position_at_end(unmatched_bb);
            self.builder.build_unconditional_branch(handle_done_bb)?;
        }

        // --- handle_done: pop handler frame, read result ---
        self.builder.position_at_end(handle_done_bb);

        let clear_fn = self.declare_runtime_effect_clear();
        self.builder.build_call(clear_fn, &[], "")?;

        if has_dispatch {
            let pop_fn = self.declare_runtime_effect_handler_stack_pop();
            self.builder
                .build_call(pop_fn, &[handler_frame_ptr.into()], "")?;
        }

        // 7. Read the handle result from the frame.
        // Check state_tag for completion mode.
        let state_tag_gep_post = self.builder.build_struct_gep(
            frame_layout.frame_type,
            frame_ptr,
            frame_layout.state_tag_index(),
            "post_state_tag_ptr",
        )?;
        let post_state_tag = self
            .builder
            .build_load(
                self.context.i32_type(),
                state_tag_gep_post,
                "post_state_tag",
            )?
            .into_int_value();

        // TODO: if state_tag == FUNCTION_RETURNED, propagate early return to
        // the enclosing function instead of treating it as normal completion.
        let _ = post_state_tag;

        self.read_result_from_frame(span, result_cg_ty, frame_ptr, &frame_layout)
    }

    // ------------------------------------------------------------------
    // Helper methods: perform op, arm body, resume payload
    // ------------------------------------------------------------------

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

        // Write to TLS perform slot based on payload type.
        match payload_val.ty {
            CgTy::Unit | CgTy::Never => {
                // No payload — write op_tag with zero payload.
                let write_fn = self.declare_runtime_effect_perform_slot_write_u64();
                let zero = self.context.i64_type().const_int(0, false);
                self.builder
                    .build_call(write_fn, &[op_tag_val.into(), zero.into()], "")?;
            }
            CgTy::String | CgTy::Ref => {
                // GC ref payload — use the gc_ref variant.
                let word = self.context.i64_type().const_int(0, false);
                let gc_ref = payload_val.value.map(|v| v.into_pointer_value());
                let write_fn = self.declare_runtime_effect_perform_slot_write_u64_with_gc_ref();
                let gc_ref_val = gc_ref.unwrap_or_else(|| self.llvm_gc_i8_ptr_type().const_null());
                self.builder.build_call(
                    write_fn,
                    &[op_tag_val.into(), word.into(), gc_ref_val.into()],
                    "",
                )?;
            }
            _ => {
                // Scalar payload — coerce to u64 and write.
                let word = self.coerce_u64_word(span, payload_val)?;
                let write_fn = self.declare_runtime_effect_perform_slot_write_u64();
                self.builder
                    .build_call(write_fn, &[op_tag_val.into(), word.into()], "")?;
            }
        }

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
                            call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(type_id)),
                            ty: cg_ty,
                            ptr: slot_ptr,
                            mutable: slot.slot().mutable(),
                        },
                    );
                }
            }
        }

        // Execute the arm body. Immediate-resume arms dedicatedly rewrite the
        // tail `resume(value)` into a payload-producing expression; the actual
        // continuation resume still happens in ArmResumeMatchedSite.
        match arm.kind {
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
        }
    }

    /// Read a binder value from the TLS perform slot.
    fn read_binder_from_perform_slot(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::String | CgTy::Ref => {
                // Read GC ref from perform slot.
                let read_fn = self.declare_runtime_effect_perform_slot_read_gc_ref();
                let raw = self
                    .builder
                    .build_call(read_fn, &[], "binder_gc_ref")?
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "perform_slot_read_gc_ref return",
                        at: at.into(),
                    })?;
                self.cg_value_from_loaded(at, cg_ty, raw)
            }
            _ => {
                // Read u64 word and narrow.
                let read_fn = self.declare_runtime_effect_perform_slot_read_u64();
                let raw = self
                    .builder
                    .build_call(read_fn, &[], "binder_word")?
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "perform_slot_read_u64 return",
                        at: at.into(),
                    })?
                    .into_int_value();
                self.narrow_u64_word_to_cg_value(at, raw, cg_ty)
            }
        }
    }

    /// Write a resume payload into a continuation struct's resume_word /
    /// resume_gc_ref fields.
    fn write_resume_payload_to_continuation(
        &mut self,
        span: crate::span::Span,
        val: CgValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let cont_ty = self.llvm_continuation_struct_type();

        match val.ty {
            CgTy::Unit | CgTy::Never => {
                // No payload to write.
            }
            CgTy::String | CgTy::Ref => {
                // GC ref → continuation's resume_gc_ref (field 7).
                if let Some(raw) = val.value {
                    let gep = self.builder.build_struct_gep(
                        cont_ty,
                        cont_ptr,
                        7, // resume_gc_ref
                        "cont_resume_gc_ref",
                    )?;
                    self.builder.build_store(gep, raw)?;
                }
            }
            _ => {
                // Scalar → coerce to u64 and write to resume_word (field 6).
                let word = self.coerce_u64_word(span, val)?;
                let gep = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6, // resume_word
                    "cont_resume_word",
                )?;
                self.builder.build_store(gep, word)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::SourceFile;
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
