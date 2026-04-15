//! T3004a: State machine LLVM emitter — frame type, step function skeleton,
//! and `handle` expression entry.
//!
//! This module generates LLVM IR from a `UnifiedHandleLoweringContract`:
//! - Frame struct type (system fields + user slots)
//! - Step function with state_tag-based dispatch
//! - Handle expression entry (allocate frame, call step_fn, collect result)
//!
//! All emission decisions are driven by the state machine contract; no source
//! shapes, old scanner results, or old mode selections are consulted.

use super::*;

use super::unified_state_machine_skeleton::{
    UnifiedFrameField, UnifiedFrameSystemField, UnifiedHandleLoweringContract,
};

/// System field indices in the frame struct.
///
/// Layout:
///   field 0: state_tag   (i32)   — current state / PC
///   field 1: resume_word (i64)   — scalar resume payload
///   field 2: resume_gc_ref (ptr addrspace(1)) — GC ref resume payload
///   field 3+: optional system fields (cleanup_flag, one_shot_flag)
///   then: user slots
const FRAME_FIELD_STATE_TAG: u32 = 0;
const FRAME_FIELD_RESUME_WORD: u32 = 1;
const FRAME_FIELD_RESUME_GC_REF: u32 = 2;

/// Tracks the frame struct layout for a specific handle expression, mapping
/// `UnifiedFrameField` indices to LLVM struct field indices.
pub(super) struct FrameLayout<'ctx> {
    pub(super) frame_type: inkwell::types::StructType<'ctx>,
    /// Total number of system fields (always >= 3: state_tag, resume_word, resume_gc_ref).
    /// Used by `user_slot_llvm_index` in T3004b for frame slot access.
    #[allow(dead_code)]
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
    /// Used by T3004b for frame slot GEP access.
    #[allow(dead_code)]
    pub(super) fn user_slot_llvm_index(&self, unified_field_index: usize) -> u32 {
        self.system_field_count + unified_field_index as u32
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
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

    /// Generate the step function for a state machine.
    ///
    /// The step function has the continuation step signature:
    ///   `void step_fn(ptr %state, i64 %resume_word, ptr %resume_gc_ref)`
    ///
    /// On entry it stores resume_word and resume_gc_ref into the frame, loads
    /// state_tag, and dispatches via `switch` to per-state basic blocks.
    ///
    /// In this skeleton phase (T3004a), each state block simply returns void.
    /// Actual op emission is deferred to T3004b.
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

        // Save caller's builder position.
        let saved_block = self.builder.get_insert_block();

        // --- Generate step function body ---
        {
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

            let state_bbs: Vec<_> = states
                .iter()
                .map(|state| {
                    let label = format!("state_{}", state.id());
                    self.context.append_basic_block(step_fn, &label)
                })
                .collect();

            // Build the switch dispatch.
            let i32_ty = self.context.i32_type();
            let cases: Vec<_> = states
                .iter()
                .zip(state_bbs.iter())
                .map(|(state, &bb)| {
                    let tag = i32_ty.const_int(state.id() as u64, false);
                    (tag, bb)
                })
                .collect();
            self.builder.build_switch(state_tag, unreachable_bb, &cases)?;

            // Emit unreachable block.
            self.builder.position_at_end(unreachable_bb);
            self.builder.build_unreachable()?;

            // T3004a skeleton: each state block returns void.
            // T3004b will replace these with actual op emission.
            for &bb in &state_bbs {
                self.builder.position_at_end(bb);
                self.builder.build_return(None)?;
            }
        }

        // Restore caller's builder position.
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        Ok(step_fn)
    }

    /// Implement `handle` expression codegen via the unified state machine.
    ///
    /// Flow:
    /// 1. Build the unified lowering contract from the `handle` HIR.
    /// 2. Generate the frame struct type and step function.
    /// 3. Allocate the frame on the heap (via malloc for now; GC alloc in T3004c).
    /// 4. Initialize the frame's state_tag to the entry state.
    /// 5. Call the step function for the initial body execution.
    /// 6. Return the handle expression result (placeholder default value for now).
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

        // 3. Allocate the frame on the heap.
        //
        // T3004a uses malloc for simplicity. T3004c will switch to GC-managed
        // allocation so the frame survives across suspension boundaries.
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

        // 6. Return the handle expression result.
        //
        // T3004a returns a default value for the handle's result type.
        // T3004b will read the actual result from the frame or state machine.
        let result_cg_ty = expected
            .or_else(|| self.cg_ty_of(contract.result_ty()))
            .unwrap_or(CgTy::Unit);
        let result = self.default_value(span, result_cg_ty)?;
        Ok(result)
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
