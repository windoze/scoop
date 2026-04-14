//! effect codegen（T0102e：从 `codegen/mod.rs` 拆分）。
//!
//! 当前阶段只保留 unified state-machine plan / segment 主线所需的最小骨架。
//! 旧的 shape-based 分流与配套 helper 已删除；后续 lowering 只能从统一元数据重新接回。

use super::*;

/// effect payload 共享的双通道 ABI 载体。
#[derive(Debug, Clone, Copy)]
struct AbiPayloadTransport<'ctx> {
    word: IntValue<'ctx>,
    gc_ref: Option<PointerValue<'ctx>>,
}

include!("state_machine_plan.rs");
include!("state_machine_segments.rs");
include!("state_machine_transform.rs");

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        _op: &hir::EffectOpRef,
        _args: &[hir::CallArg],
        _expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "effect perform codegen is temporarily unavailable until unified lowering is reconnected",
            at: span.into(),
        })
    }

    pub(super) fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        _handle: &hir::HandleExpr,
        _expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "effect handle codegen is temporarily unavailable until unified lowering is reconnected",
            at: span.into(),
        })
    }

    pub(super) fn decode_abi_payload_transport(
        &mut self,
        at: crate::span::Span,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                self.decode_u64_word_to_cg_value(at, word, ty)
            }
            CgTy::String => {
                let str_ptr_ty = self.llvm_scoop_string_ptr_type();
                let s =
                    self.builder
                        .build_pointer_cast(gc_ref, str_ptr_ty, "abi_payload_string")?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(s.into()),
                })
            }
            CgTy::Ref => Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(gc_ref.into()),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "aggregate abi payload decode is temporarily unavailable until unified lowering is reconnected",
                    at: at.into(),
                })
            }
        }
    }

    pub(super) fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let rt = self.declare_runtime_effect_is_active();
        let call = self.builder.build_call(rt, &[], "effect_is_active")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return value",
                at: at.into(),
            })?;
        let BasicValueEnum::IntValue(active_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return type",
                at: at.into(),
            });
        };
        Ok(self.builder.build_int_compare(
            IntPredicate::NE,
            active_i32,
            self.context.i32_type().const_zero(),
            "effect_active",
        )?)
    }

    pub(super) fn callee_suspend_first_local_field_index() -> u32 {
        3
    }

    pub(super) fn get_or_create_callee_suspend_state_type(
        &mut self,
        at: crate::span::Span,
        state_ty_name: &str,
        saved_locals: &[CalleeSuspendLocal],
    ) -> Result<inkwell::types::StructType<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.context.get_struct_type(state_ty_name) {
            return Ok(existing);
        }

        let ty = self.context.opaque_struct_type(state_ty_name);
        let header_ty = self.llvm_gc_object_header_type();
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
        fields.push(header_ty.into());
        fields.push(i64_ty.into());
        fields.push(gc_i8_ptr_ty.into());
        for local in saved_locals {
            fields.push(match local.cg_ty {
                CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => i64_ty.into(),
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "callee suspend local type",
                        at: at.into(),
                    });
                }
            });
        }
        ty.set_body(&fields, false);
        Ok(ty)
    }

    pub(super) fn emit_effect_unwind_if_active(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let cont_bb = self.context.append_basic_block(func, "effect_unwind_cont");
        let is_active = self.emit_effect_is_active_i1(at)?;

        if let Some(target) = self.raise_target_stack.last().copied() {
            self.builder
                .build_conditional_branch(is_active, target, cont_bb)?;
        } else {
            let ret_bb = self
                .context
                .append_basic_block(func, "effect_unwind_return");
            self.builder
                .build_conditional_branch(is_active, ret_bb, cont_bb)?;

            self.builder.position_at_end(ret_bb);
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect unwind needs function return type",
                    at: at.into(),
                })?;
            let v = self.default_value(at, ret_ty)?;
            self.emit_return(at, ret_ty, v)?;
        }

        self.builder.position_at_end(cont_bb);
        Ok(())
    }

    pub(super) fn fun_ty_effects_is_pure(&self, ty: TypeId) -> Option<bool> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => Some(fun_ty.effects.is_pure()),
            _ => None,
        }
    }

    pub(super) fn emit_raise_runtime_error_variant(
        &mut self,
        span: crate::span::Span,
        _variant: &str,
    ) -> Result<(), LlvmEmitError> {
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "runtime error raise helper is temporarily unavailable until unified lowering is reconnected",
            at: span.into(),
        })
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

    pub(super) fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        _fqn: &str,
        _args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "sysroot effect intrinsics are temporarily unavailable until unified lowering is reconnected",
            at: span.into(),
        })
    }
}
