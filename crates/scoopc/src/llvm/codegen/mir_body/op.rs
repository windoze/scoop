//! MIR unary / binary operator lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        operand: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let value = operand
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::bool(
                    self.builder.build_not(value, "pass_mir_not")?,
                ))
            }
            ast::UnaryOp::Neg => {
                if let Some((value, int_ty)) = operand.as_int() {
                    return Ok(CgValue::int(
                        self.builder.build_int_neg(value, "pass_mir_neg")?,
                        int_ty,
                    ));
                }
                if let Some((value, float_ty)) = operand.as_float() {
                    return Ok(CgValue::float(
                        self.builder.build_float_neg(value, "pass_mir_fneg")?,
                        float_ty,
                    ));
                }
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR numeric unary",
                    at: span.into(),
                })
            }
            ast::UnaryOp::BitNot => {
                let (value, int_ty) =
                    operand.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::int(
                    self.builder.build_not(value, "pass_mir_bitnot")?,
                    int_ty,
                ))
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: CgValue<'ctx>,
        rhs: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_int(), rhs.as_int()) {
            let target_ty = self.pass_mir_binary_int_target_ty(op, l_ty, r_ty);
            let l = self.cast_int(l, l_ty, target_ty)?;
            let r = self.cast_int(r, r_ty, target_ty)?;
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::int(
                        self.builder.build_int_add(l, r, "pass_mir_iadd")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::int(
                        self.builder.build_int_sub(l, r, "pass_mir_isub")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::int(
                        self.builder.build_int_mul(l, r, "pass_mir_imul")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div if target_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_div(l, r, "pass_mir_sdiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_div(l, r, "pass_mir_udiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem if target_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_rem(l, r, "pass_mir_srem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_rem(l, r, "pass_mir_urem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shl => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder.build_left_shift(l, r, "pass_mir_shl")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shr if target_ty.signed => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, true, "pass_mir_ashr")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Shr => {
                    let r = self.mask_shift_count(target_ty, r)?;
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, false, "pass_mir_lshr")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitAnd => {
                    return Ok(CgValue::int(
                        self.builder.build_and(l, r, "pass_mir_iand")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitXor => {
                    return Ok(CgValue::int(
                        self.builder.build_xor(l, r, "pass_mir_ixor")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::BitOr => {
                    return Ok(CgValue::int(
                        self.builder.build_or(l, r, "pass_mir_ior")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Lt => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Lt),
                    l,
                    r,
                    "pass_mir_ilt",
                )?,
                ast::BinaryOp::Le => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Le),
                    l,
                    r,
                    "pass_mir_ile",
                )?,
                ast::BinaryOp::Gt => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Gt),
                    l,
                    r,
                    "pass_mir_igt",
                )?,
                ast::BinaryOp::Ge => self.builder.build_int_compare(
                    int_predicate(target_ty, IntCompareKind::Ge),
                    l,
                    r,
                    "pass_mir_ige",
                )?,
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_ieq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_ine")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some(l), Some(r)) = (lhs.as_bool(), rhs.as_bool()) {
            let value = match op {
                ast::BinaryOp::LogAnd | ast::BinaryOp::BitAnd => {
                    self.builder.build_and(l, r, "pass_mir_band")?
                }
                ast::BinaryOp::LogOr | ast::BinaryOp::BitOr => {
                    self.builder.build_or(l, r, "pass_mir_bor")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_beq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_bne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_float(), rhs.as_float()) {
            let target_ty = if l_ty == CgTy::Float64 || r_ty == CgTy::Float64 {
                CgTy::Float64
            } else {
                CgTy::Float32
            };
            let l = self.cast_float(l, l_ty, target_ty)?;
            let r = self.cast_float(r, r_ty, target_ty)?;
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::float(
                        self.builder.build_float_add(l, r, "pass_mir_fadd")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::float(
                        self.builder.build_float_sub(l, r, "pass_mir_fsub")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::float(
                        self.builder.build_float_mul(l, r, "pass_mir_fmul")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::float(
                        self.builder.build_float_div(l, r, "pass_mir_fdiv")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::float(
                        self.builder.build_float_rem(l, r, "pass_mir_frem")?,
                        target_ty,
                    ));
                }
                ast::BinaryOp::Lt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLT, l, r, "pass_mir_flt")?
                }
                ast::BinaryOp::Le => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLE, l, r, "pass_mir_fle")?
                }
                ast::BinaryOp::Gt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGT, l, r, "pass_mir_fgt")?
                }
                ast::BinaryOp::Ge => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGE, l, r, "pass_mir_fge")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_float_compare(FloatPredicate::OEQ, l, r, "pass_mir_feq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_float_compare(FloatPredicate::UNE, l, r, "pass_mir_fne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR float binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if matches!(op, ast::BinaryOp::Eq | ast::BinaryOp::Ne)
            && lhs.ty == CgTy::String
            && rhs.ty == CgTy::String
        {
            let Some(BasicValueEnum::PointerValue(lhs_ptr)) = lhs.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality lhs",
                    at: span.into(),
                });
            };
            let Some(BasicValueEnum::PointerValue(rhs_ptr)) = rhs.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality rhs",
                    at: span.into(),
                });
            };
            let runtime = self.declare_runtime_string_equals();
            let call = self.builder.build_call(
                runtime,
                &[lhs_ptr.into(), rhs_ptr.into()],
                "pass_mir_string_eq",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR string equality return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(eq_i64) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR string equality return type",
                    at: span.into(),
                });
            };
            let mut is_eq = self.builder.build_int_compare(
                IntPredicate::NE,
                eq_i64,
                self.context.i64_type().const_zero(),
                "pass_mir_string_eq_bool",
            )?;
            if op == ast::BinaryOp::Ne {
                is_eq = self.builder.build_not(is_eq, "pass_mir_string_ne_bool")?;
            }
            return Ok(CgValue::bool(is_eq));
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR binary operands",
            at: span.into(),
        })
    }

    pub(in crate::llvm::codegen) fn pass_mir_binary_int_target_ty(
        &self,
        op: ast::BinaryOp,
        lhs: IntTy,
        rhs: IntTy,
    ) -> IntTy {
        if matches!(op, ast::BinaryOp::Shl | ast::BinaryOp::Shr) {
            return lhs;
        }
        let word_bits = self.host.word_bit_width();
        if lhs.bits == word_bits && rhs.bits != word_bits {
            rhs
        } else {
            lhs
        }
    }

    pub(in crate::llvm::codegen) fn mir_local_slot(
        &self,
        span: crate::span::Span,
        slots: &[MirLocalSlot<'ctx>],
        local: crate::mir::LocalId,
    ) -> Result<MirLocalSlot<'ctx>, LlvmEmitError> {
        slots
            .get(local.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR local",
                at: span.into(),
            })
    }

    pub(in crate::llvm::codegen) fn load_mir_local(
        &mut self,
        span: crate::span::Span,
        slot: MirLocalSlot<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match slot.cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let local_ptr = self.local_ptr_for_use(
                    span,
                    CgLocal {
                        hir_ty: None,
                        call_may_suspend: false,
                        ty: slot.cg_ty,
                        ptr: slot.ptr,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                    "pass_mir_load_slot",
                )?;
                let llvm_ty = self.llvm_basic_type_of(span, slot.cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "pass_mir_load")?;
                self.cg_value_from_loaded(span, slot.cg_ty, loaded)
            }
        }
    }
}
