//! effect codegen（T0102e：从 `codegen/mod.rs` 拆分）。
//!
//! 统一 state-machine 主线 effect codegen：
//! - plan builder / segment / transform 骨架在 `unified_state_machine_skeleton` 模块
//! - LLVM emitter 在 `state_machine_emitter.rs`（T3004a+）
//! - 旧的分流与配套 helper 已删除；lowering 只从统一合同出发。

use super::*;

// T3004a：state machine LLVM emitter — 从 UnifiedHandleLoweringContract 生成
// LLVM IR（frame type、step function、handle 入口）。
mod state_machine_emitter;

// T2999/T3002：统一 state-machine 骨架是后续 effect LLVM lowering 的唯一候选合同。
// 内部实现细节（plan builder、segment 投影、validation helper 等）保留在
// `unified_state_machine_skeleton` 模块内，该模块整体 #[allow(dead_code)] 因为
// 生产入口尚未完整接线。
//
// T3002 变更：核心类型（HandleStateMachinePlan、HandleSegmentList、
// UnifiedHandleStateMachine）现在从模块中 re-export 出来，以便 T3003+ 的
// 生产代码可以直接引用。每个 re-export 带有独立的 #[allow(dead_code)]，
// 在后续 lowering 接线时逐个移除即可；不再被 blanket dead_code 遮蔽。
#[allow(dead_code)]
mod unified_state_machine_skeleton {
    use super::*;

    include!("state_machine_plan.rs");
    include!("state_machine_segments.rs");
    include!("state_machine_transform.rs");
}

// 后续 T3003+ 的统一 lowering 合同将消费这些类型。每个 re-export 的
// #[allow(unused_imports)] 在该类型被生产入口实际引用后移除。
#[allow(unused_imports)]
pub(super) use unified_state_machine_skeleton::HandleStateMachinePlan;
#[allow(unused_imports)]
pub(super) use unified_state_machine_skeleton::HandleSegmentList;
#[allow(unused_imports)]
pub(super) use unified_state_machine_skeleton::UnifiedHandleStateMachine;
#[allow(unused_imports)]
pub(super) use unified_state_machine_skeleton::UnifiedHandleLoweringContract;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Emit code for a standalone `perform` expression (outside of a state
    /// machine step function).  Writes the op_tag + payload to the TLS
    /// perform slot, sets the active flag, and returns a default value.
    /// The caller's state machine (via SuspendCall + Suspend terminator) will
    /// detect the active flag and handle dispatch.
    pub(super) fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let op_tag = self.effect_op_tag(&op.fqn);
        let op_tag_val = self
            .context
            .i32_type()
            .const_int(op_tag as u64, false);

        // Evaluate the payload from the first positional/named arg (if any).
        let payload_val = if args.is_empty() {
            CgValue::unit()
        } else {
            let arg_expr = match &args[0] {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            self.codegen_expr_in_expected_context(arg_expr, None)?
        };

        // Write to TLS perform slot (same logic as state machine emit_perform_op).
        match payload_val.ty {
            CgTy::Unit | CgTy::Never => {
                let write_fn = self.declare_runtime_effect_perform_slot_write_u64();
                let zero = self.context.i64_type().const_int(0, false);
                self.builder.build_call(
                    write_fn,
                    &[op_tag_val.into(), zero.into()],
                    "",
                )?;
            }
            CgTy::String | CgTy::Ref => {
                let word = self.context.i64_type().const_int(0, false);
                let gc_ref = payload_val.value.map(|v| v.into_pointer_value());
                let write_fn =
                    self.declare_runtime_effect_perform_slot_write_u64_with_gc_ref();
                let gc_ref_val =
                    gc_ref.unwrap_or_else(|| self.llvm_gc_i8_ptr_type().const_null());
                self.builder.build_call(
                    write_fn,
                    &[op_tag_val.into(), word.into(), gc_ref_val.into()],
                    "",
                )?;
            }
            _ => {
                let word = self.coerce_u64_word(span, payload_val)?;
                let write_fn = self.declare_runtime_effect_perform_slot_write_u64();
                self.builder.build_call(
                    write_fn,
                    &[op_tag_val.into(), word.into()],
                    "",
                )?;
            }
        }

        // Set the TLS active flag to signal that an effect was performed.
        let set_active = self.declare_runtime_effect_set_active();
        self.builder.build_call(set_active, &[], "")?;

        // Return a default value for the expected type.  The actual resume
        // value will be provided by the handler; this default propagates
        // through intermediate frames until the state machine catches it.
        let result_ty = expected.unwrap_or(CgTy::Unit);
        self.default_value(span, result_ty)
    }

    pub(super) fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_handle_expr_via_state_machine(span, handle, expected)
    }

    /// Emit code to raise a runtime error variant through the effect system.
    /// Writes the `Raise.raise` op_tag to the TLS perform slot, sets the
    /// active flag, and returns.  The caller is responsible for subsequent
    /// control flow (dead block / unreachable).
    pub(super) fn emit_raise_runtime_error_variant(
        &mut self,
        _span: crate::span::Span,
        _variant: &str,
    ) -> Result<(), LlvmEmitError> {
        // Use the well-known Raise.raise FQN (op_tag = 1 by convention).
        let op_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let op_tag_val = self
            .context
            .i32_type()
            .const_int(op_tag as u64, false);

        // Write a zero payload (the variant name is not yet part of the
        // runtime payload protocol — this is a minimal implementation that
        // signals "a Raise happened" without encoding the variant).
        let write_fn = self.declare_runtime_effect_perform_slot_write_u64();
        let zero = self.context.i64_type().const_int(0, false);
        self.builder.build_call(
            write_fn,
            &[op_tag_val.into(), zero.into()],
            "",
        )?;

        // Set the TLS active flag.
        let set_active = self.declare_runtime_effect_set_active();
        self.builder.build_call(set_active, &[], "")?;

        Ok(())
    }

    pub(super) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(i64_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i64_ty, "bool_to_u64")?)
            }
            CgTy::Int(_) => {
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 64,
                    signed: false,
                };
                Ok(self.cast_int(raw, from, to)?)
            }
            CgTy::Float64 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float64",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_bit_cast(raw, i64_ty, "f64_to_u64_bits")?
                    .into_int_value())
            }
            CgTy::Float32 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float32",
                    at: at.into(),
                })?;
                let bits32 = self
                    .builder
                    .build_bit_cast(raw, self.context.i32_type(), "f32_to_u32_bits")?
                    .into_int_value();
                Ok(self
                    .builder
                    .build_int_z_extend(bits32, i64_ty, "u32_to_u64_bits")?)
            }
            CgTy::String | CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "u64 word from gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from composite value",
                    at: at.into(),
                })
            }
        }
    }

    fn effect_intrinsic_word_int_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        }
    }

    fn codegen_sysroot_effect_intrinsic_word_arg(
        &mut self,
        span: crate::span::Span,
        arg: &hir::CallArg,
        kind: &'static str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let hir::CallArg::Positional(expr) = arg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        };

        let word_ty = self.effect_intrinsic_word_int_ty();
        let value = self.codegen_expr_in_expected_context(expr, Some(CgTy::Int(word_ty)))?;
        let value = self.coerce_value(expr.span, value, CgTy::Int(word_ty))?;
        let (raw, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind,
            at: expr.span.into(),
        })?;
        Ok(raw)
    }

    pub(super) fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let word_ty = self.effect_intrinsic_word_int_ty();
        let op_tag_ty = IntTy {
            bits: 32,
            signed: false,
        };
        let slot_word_ty = IntTy {
            bits: 64,
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_effect_is_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_is_active();
                let call = self.builder.build_call(rt, &[], "effect_is_active")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(active_i32) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return type",
                        at: span.into(),
                    });
                };
                let active_word = self.cast_int(active_i32, op_tag_ty, word_ty)?;
                Ok(CgValue::int(active_word, word_ty))
            }
            "scoop.core.__scoop_effect_set_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect set_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_set_active();
                let _ = self.builder.build_call(rt, &[], "effect_set_active")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_clear" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect clear arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_clear();
                let _ = self.builder.build_call(rt, &[], "effect_clear")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write arity mismatch",
                        at: span.into(),
                    });
                }

                let op_tag_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_write op_tag",
                )?;
                let value_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[1],
                    "effect slot_write value",
                )?;
                let op_tag = self.cast_int(op_tag_word, word_ty, op_tag_ty)?;
                let value = self.cast_int(value_word, word_ty, slot_word_ty)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64();
                let _ = self.builder.build_call(
                    rt,
                    &[op_tag.into(), value.into()],
                    "effect_slot_write_u64",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write2" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 arity mismatch",
                        at: span.into(),
                    });
                }

                let op_tag_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_write2 op_tag",
                )?;
                let word0_raw = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[1],
                    "effect slot_write2 word0",
                )?;
                let word1_raw = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[2],
                    "effect slot_write2 word1",
                )?;
                let op_tag = self.cast_int(op_tag_word, word_ty, op_tag_ty)?;
                let word0 = self.cast_int(word0_raw, word_ty, slot_word_ty)?;
                let word1 = self.cast_int(word1_raw, word_ty, slot_word_ty)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64_2();
                let _ = self.builder.build_call(
                    rt,
                    &[op_tag.into(), word0.into(), word1.into()],
                    "effect_slot_write_u64_2",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_read_op_tag" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_op_tag();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_op_tag")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(op_tag) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return type",
                        at: span.into(),
                    });
                };
                let op_tag_word = self.cast_int(op_tag, op_tag_ty, word_ty)?;
                Ok(CgValue::int(op_tag_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_len_words" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_len_words")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(len_words) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return type",
                        at: span.into(),
                    });
                };
                let len_word = self.cast_int(len_words, op_tag_ty, word_ty)?;
                Ok(CgValue::int(len_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_value" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_u64();
                let call = self.builder.build_call(rt, &[], "effect_slot_read_u64")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return type",
                        at: span.into(),
                    });
                };
                let value_word = self.cast_int(value_u64, slot_word_ty, word_ty)?;
                Ok(CgValue::int(value_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_word" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word arity mismatch",
                        at: span.into(),
                    });
                }

                let index_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_read_word index",
                )?;
                let index = self.cast_int(index_word, word_ty, op_tag_ty)?;
                let rt = self.declare_runtime_effect_perform_slot_read_u64_at();
                let call =
                    self.builder
                        .build_call(rt, &[index.into()], "effect_slot_read_u64_at")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return type",
                        at: span.into(),
                    });
                };
                let value_word = self.cast_int(value_u64, slot_word_ty, word_ty)?;
                Ok(CgValue::int(value_word, word_ty))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot effect intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }
}
