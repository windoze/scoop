//! State machine LLVM emitter — frame type, step function, and `handle`
//! expression entry.
//!
//! This module generates LLVM IR from a `UnifiedHandleLoweringContract`:
//! - Frame struct type (system fields + user slots)
//! - Step function with state_tag-based dispatch, per-state op emission,
//!   and basic terminators (Goto, Branch, ReturnHandle, ReturnFromFunction)
//! - Handle expression entry (allocate frame, call step_fn, collect result)
//!
//! T3004a: skeleton — frame type, step function with placeholder state blocks,
//!         handle expression entry.
//! T3004b: op emission per state block, frame slot GEP read/write, basic
//!         terminators, result passing through frame.
//!
//! All emission decisions are driven by the state machine contract; no source
//! shapes, old scanner results, or old mode selections are consulted.

use std::collections::HashMap;

use super::*;

use super::unified_state_machine_skeleton::{
    HandleBranchCondition, HandleStateOp, UnifiedFrameField, UnifiedFrameSystemField,
    UnifiedHandleLoweringContract, UnifiedState, UnifiedStateTerminator,
};

/// System field indices in the frame struct.
///
/// Layout:
///   field 0: state_tag   (i32)   — current state / PC
///   field 1: resume_word (i64)   — scalar resume payload / handle result word
///   field 2: resume_gc_ref (ptr addrspace(1)) — GC ref resume payload / handle result ref
///   field 3+: optional system fields (cleanup_flag, one_shot_flag)
///   then: user slots
const FRAME_FIELD_STATE_TAG: u32 = 0;
const FRAME_FIELD_RESUME_WORD: u32 = 1;
const FRAME_FIELD_RESUME_GC_REF: u32 = 2;

/// Sentinel state_tag value: the handle body has finished and its result is
/// available in resume_word / resume_gc_ref.  This is the normal completion
/// path — `ReturnHandle` sets this.
const STATE_TAG_HANDLE_RETURNED: u32 = 0xFFFF_FFFE;

/// Sentinel state_tag value: an early `return` statement inside the handle
/// body wants to return from the *enclosing function*, not just the handle.
/// The handle entry reads this and propagates the return upward.
const STATE_TAG_FUNCTION_RETURNED: u32 = 0xFFFF_FFFF;

/// Tracks the frame struct layout for a specific handle expression, mapping
/// `UnifiedFrameField` indices to LLVM struct field indices.
pub(super) struct FrameLayout<'ctx> {
    pub(super) frame_type: inkwell::types::StructType<'ctx>,
    /// Total number of system fields (always >= 3: state_tag, resume_word, resume_gc_ref).
    system_field_count: u32,
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

    /// Return the LLVM struct field index for a user slot given its
    /// `UnifiedFrameSlot::field_index()`.
    pub(super) fn user_slot_llvm_index(&self, unified_field_index: usize) -> u32 {
        self.system_field_count + unified_field_index as u32
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    // ------------------------------------------------------------------
    // Frame layout generation (T3004a)
    // ------------------------------------------------------------------

    /// Generate the LLVM struct type for a state machine frame.
    ///
    /// The frame struct layout follows `UnifiedFrameSchema`:
    /// - System fields: state_tag (i32), resume_word (i64), resume_gc_ref (ptr),
    ///   and optionally cleanup_flag (i32), one_shot_flag (i32).
    /// - User slots: one field per `UnifiedFrameSlot`, typed according to the
    ///   slot's `TypeId`.
    fn emit_effect_frame_layout(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
    ) -> Result<FrameLayout<'ctx>, LlvmEmitError> {
        let frame = contract.frame();
        let system_fields = frame.fields();

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();

        // Build the system field types in declaration order.
        let mut field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = Vec::new();

        // Always present: state_tag, resume_word, resume_gc_ref.
        field_types.push(i32_ty.into());    // state_tag
        field_types.push(i64_ty.into());    // resume_word
        field_types.push(gc_ptr_ty.into()); // resume_gc_ref

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

        let system_field_count = field_types.len() as u32;

        // User slots: one LLVM field per UnifiedFrameSlot, in field_index order.
        let mut user_slots: Vec<(usize, crate::ty::TypeId)> = frame
            .slots()
            .iter()
            .map(|slot| (slot.field_index(), slot.slot().ty()))
            .collect();
        user_slots.sort_by_key(|(idx, _)| *idx);

        for (_idx, type_id) in &user_slots {
            let cg_ty =
                self.cg_ty_of(*type_id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect frame slot type",
                        at: span.into(),
                    })?;
            let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
            field_types.push(llvm_ty);
        }

        // Create the named struct type.
        let type_name = format!(
            "scoop.effect.frame.{:x}",
            span.start ^ (span.end << 16)
        );
        let frame_type = self.context.opaque_struct_type(&type_name);
        frame_type.set_body(&field_types, false);

        Ok(FrameLayout {
            frame_type,
            system_field_count,
        })
    }

    // ------------------------------------------------------------------
    // Step function generation (T3004a skeleton + T3004b op emission)
    // ------------------------------------------------------------------

    /// Generate the step function for a state machine.
    ///
    /// The step function has the continuation step signature:
    ///   `void step_fn(ptr %state, i64 %resume_word, ptr %resume_gc_ref)`
    ///
    /// On entry it stores resume_word and resume_gc_ref into the frame, loads
    /// state_tag, and dispatches via `switch` to per-state basic blocks.
    ///
    /// T3004b: each state block emits its ops and terminates with the
    /// appropriate LLVM terminator (branch, return, etc.).
    fn emit_effect_step_function(
        &mut self,
        span: crate::span::Span,
        contract: &UnifiedHandleLoweringContract,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, LlvmEmitError> {
        // Step function signature: (ptr, i64, ptr) -> void
        let state_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();

        let param_tys: [inkwell::types::BasicMetadataTypeEnum<'ctx>; 3] = [
            state_ptr_ty.into(),
            i64_ty.into(),
            gc_ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);

        let fn_name = format!(
            "scoop.effect.step.{:x}",
            span.start ^ (span.end << 16)
        );
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
        let unreachable_bb =
            self.context.append_basic_block(step_fn, "unreachable");

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
        self.builder.build_switch(state_tag, unreachable_bb, &cases)?;

        // Emit unreachable block.
        self.builder.position_at_end(unreachable_bb);
        self.builder.build_unreachable()?;

        // T3004b: emit ops and terminators for each state block.
        for state in states {
            let bb = state_bb_map[&state.id()];
            self.builder.position_at_end(bb);

            // Fresh env scope for this state's locals.
            self.env.push_scope();

            let last_value = self.emit_state_ops(
                span,
                state,
                state_ptr,
                frame_layout,
                contract,
            )?;

            self.emit_state_terminator(
                span,
                state.terminator(),
                last_value,
                state_ptr,
                frame_layout,
                &state_bb_map,
            )?;

            self.env.pop_scope();
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Per-state op emission (T3004b)
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
                HandleStateOp::CleanupEdgeComplete
                | HandleStateOp::ReturnToEnclosingExpression => {
                    // T3004d: cleanup scope ops — placeholder for now.
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
                        let val =
                            self.codegen_expr_in_expected_context(init, None)?;
                        last_value = Some(val);
                    }
                }

                // --- Expression ops: delegate to existing codegen ---
                HandleStateOp::Literal { expr }
                | HandleStateOp::VarRef { expr }
                | HandleStateOp::StructLit { expr }
                | HandleStateOp::TupleLit { expr }
                | HandleStateOp::InterpolatedString { expr }
                | HandleStateOp::Expr { expr }
                | HandleStateOp::BinaryExpr { expr }
                | HandleStateOp::Call { expr }
                | HandleStateOp::WhenExpr { expr }
                | HandleStateOp::Closure { expr } => {
                    let val =
                        self.codegen_expr_in_expected_context(expr, None)?;
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
                            let val = self.codegen_expr_in_expected_context(
                                val_expr, None,
                            )?;
                            last_value = Some(val);
                        } else {
                            last_value = Some(CgValue::unit());
                        }
                    }
                }

                // --- Break/Continue: control flow handled by terminator ---
                HandleStateOp::Break { .. }
                | HandleStateOp::Continue { .. } => {
                    // The state machine represents break/continue as state
                    // transitions (Goto terminator).  No LLVM IR needed here.
                }

                // --- Suspend-related ops: T3004c ---
                HandleStateOp::SuspendCall { .. }
                | HandleStateOp::Perform { .. }
                | HandleStateOp::ResumeAfterSite { .. }
                | HandleStateOp::ObjectInitAccessBoundary { .. }
                | HandleStateOp::RuntimeRaiseBoundary { .. } => {
                    // T3004c: suspend/resume ops — return void for now.
                    self.builder.build_return(None)?;
                    return Ok(last_value);
                }

                // --- Arm / nested handle ops: T3004c/d ---
                HandleStateOp::ExecuteArmBody { .. }
                | HandleStateOp::NestedHandle { .. }
                | HandleStateOp::NestedHandleBoundary { .. } => {
                    // T3004c/d: handler arm and nested handle — return void.
                    self.builder.build_return(None)?;
                    return Ok(last_value);
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
        let target_ty =
            self.cg_ty_of(decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bind local type in state machine",
                    at: decl.span.into(),
                })?;

        // Evaluate initializer.
        let init_val = match decl.init.as_ref() {
            Some(init_expr) => {
                self.codegen_initializer_expr(init_expr, target_ty, decl.ty)?
            }
            None => self.default_value(span, target_ty)?,
        };

        // Find the frame slot for this local.
        let field_index = contract
            .frame()
            .get_slot_field_index(id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bind local: frame slot not found",
                at: decl.span.into(),
            })?;
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
        let field_index = contract
            .frame()
            .get_slot_field_index(id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "read local: frame slot not found",
                at: at.into(),
            })?;
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
        let cg_ty =
            self.cg_ty_of(type_id)
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
                ty: cg_ty,
                ptr: slot_ptr,
                mutable: unified_slot.slot().mutable(),
            },
        );

        // Load and return.
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_ty, slot_ptr, "slot_val")?;
        self.cg_value_from_loaded(at, cg_ty, loaded)
    }

    /// Emit a statement-type op (Assign, etc.) by dispatching to existing
    /// statement codegen.  The env must already contain the referenced locals
    /// (via prior BindLocal / ReadLocal ops in the same state).
    fn emit_stmt_op(
        &mut self,
        stmt: &hir::Stmt,
    ) -> Result<(), LlvmEmitError> {
        match &stmt.kind {
            hir::StmtKind::Empty => Ok(()),
            hir::StmtKind::Val(decl) => self.codegen_val_decl(decl),
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)
            }
            hir::StmtKind::Expr(expr) => {
                let _ =
                    self.codegen_expr_in_expected_context(expr, Some(CgTy::Unit))?;
                Ok(())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unsupported statement in state machine",
                at: stmt.span.into(),
            }),
        }
    }

    // ------------------------------------------------------------------
    // Per-state terminator emission (T3004b)
    // ------------------------------------------------------------------

    /// Emit the LLVM terminator for a state block.
    fn emit_state_terminator(
        &mut self,
        span: crate::span::Span,
        terminator: &UnifiedStateTerminator,
        last_value: Option<CgValue<'ctx>>,
        state_ptr: PointerValue<'ctx>,
        frame_layout: &FrameLayout<'ctx>,
        state_bb_map: &HashMap<u32, inkwell::basic_block::BasicBlock<'ctx>>,
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
                let target_bb =
                    self.lookup_state_bb(*next_state, state_bb_map, span)?;
                self.builder.build_unconditional_branch(target_bb)?;
            }

            UnifiedStateTerminator::Branch {
                condition,
                then_state,
                else_state,
                ..
            } => {
                let cond_bool = self.emit_branch_condition(condition)?;
                let then_bb =
                    self.lookup_state_bb(*then_state, state_bb_map, span)?;
                let else_bb =
                    self.lookup_state_bb(*else_state, state_bb_map, span)?;
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

            // T3004c/d terminators — placeholder: return void.
            UnifiedStateTerminator::Suspend { .. }
            | UnifiedStateTerminator::CleanupEnter { .. }
            | UnifiedStateTerminator::ArmReturnHandle
            | UnifiedStateTerminator::ArmResumeMatchedSite
            | UnifiedStateTerminator::ArmMaterializeContinuation => {
                self.builder.build_return(None)?;
            }
        }

        Ok(())
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
        let val =
            self.codegen_expr_in_expected_context(expr, Some(CgTy::Bool))?;
        val.as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let val = self
            .context
            .i32_type()
            .const_int(tag_value as u64, false);
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
                let loaded = self
                    .builder
                    .build_load(gc_ptr_ty, gep, "result_ref")?;
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
                let i32_trunc = self.builder.build_int_truncate(
                    word,
                    self.context.i32_type(),
                    "u64_to_u32",
                )?;
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                // Composite result transport is not yet supported via u64 word.
                // T3004c/d may extend this.
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "narrow u64 to composite type (not yet supported)",
                    at: span.into(),
                })
            }
        }
    }

    // ------------------------------------------------------------------
    // Handle expression entry (T3004a + T3004b result reading)
    // ------------------------------------------------------------------

    /// Implement `handle` expression codegen via the unified state machine.
    ///
    /// Flow:
    /// 1. Build the unified lowering contract from the `handle` HIR.
    /// 2. Generate the frame struct type and step function.
    /// 3. Allocate the frame on the heap (malloc; GC alloc in T3004c).
    /// 4. Initialize the frame's state_tag to the entry state.
    /// 5. Call the step function for the initial body execution.
    /// 6. Read the handle result from the frame (or propagate early return).
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
        let step_fn =
            self.emit_effect_step_function(span, &contract, &frame_layout)?;

        // 3. Allocate the frame on the heap.
        //
        // Uses malloc for now.  T3004c will switch to GC-managed allocation
        // so the frame survives across suspension boundaries.
        let frame_size = self
            .target_data
            .get_store_size(&frame_layout.frame_type);
        let malloc = self.declare_libc_malloc();
        let size_val = self.context.i64_type().const_int(frame_size, false);
        let raw_ptr = self
            .builder
            .build_call(malloc, &[size_val.into()], "effect_frame_raw")?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?
            .into_pointer_value();

        // 4. Initialize the frame: zero-fill then set state_tag.
        let memset = self.declare_llvm_memset();
        let i8_zero = self.context.i8_type().const_int(0, false);
        let i1_false = self.context.bool_type().const_int(0, false);
        self.builder.build_call(
            memset,
            &[
                raw_ptr.into(),
                i8_zero.into(),
                size_val.into(),
                i1_false.into(),
            ],
            "",
        )?;

        // Set state_tag to entry state.
        let state_tag_gep = self.builder.build_struct_gep(
            frame_layout.frame_type,
            raw_ptr,
            frame_layout.state_tag_index(),
            "entry_state_tag_ptr",
        )?;
        let entry_state_val = self
            .context
            .i32_type()
            .const_int(contract.entry_state() as u64, false);
        self.builder.build_store(state_tag_gep, entry_state_val)?;

        // 5. Call the step function for initial body execution.
        let i64_zero = self.context.i64_type().const_int(0, false);
        let gc_null = self.llvm_gc_i8_ptr_type().const_null();
        self.builder.build_call(
            step_fn,
            &[raw_ptr.into(), i64_zero.into(), gc_null.into()],
            "",
        )?;

        // 6. Read the handle result from the frame.
        let result_cg_ty = expected
            .or_else(|| self.cg_ty_of(contract.result_ty()))
            .unwrap_or(CgTy::Unit);

        // Check state_tag to determine completion mode.
        let state_tag_gep_post = self.builder.build_struct_gep(
            frame_layout.frame_type,
            raw_ptr,
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

        // For now, read the result regardless of completion mode.
        // T3004c will add dispatch loop and early-return propagation.
        // If state_tag == FUNCTION_RETURNED, we should propagate the early
        // return to the enclosing function, but that requires the outer
        // function's return context (handled in T3005).
        let _ = post_state_tag;

        self.read_result_from_frame(span, result_cg_ty, raw_ptr, &frame_layout)
    }

    /// Declare `llvm.memset.p0.i64` intrinsic.
    fn declare_llvm_memset(&self) -> inkwell::values::FunctionValue<'ctx> {
        const NAME: &str = "llvm.memset.p0.i64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        let void_ty = self.context.void_type();
        let i8_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i8_ty = self.context.i8_type();
        let i64_ty = self.context.i64_type();
        let i1_ty = self.context.bool_type();
        let param_tys: [inkwell::types::BasicMetadataTypeEnum<'ctx>; 4] = [
            i8_ptr_ty.into(),
            i8_ty.into(),
            i64_ty.into(),
            i1_ty.into(),
        ];
        let fn_ty = void_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }
}
