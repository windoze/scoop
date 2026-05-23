//! Equality, bool logic, coerce_value, pointer-like to option enum coercion.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_equality(
        &mut self,
        _span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let lhs_v = self.codegen_expr(lhs)?;
        if lhs_v.ty == CgTy::String {
            let deferred_lhs =
                self.defer_gc_sensitive_cg_value(lhs.span, "string_eq_lhs", lhs_v)?;
            let rhs_v = self.codegen_expr(rhs)?;
            let lhs_v =
                self.materialize_deferred_cg_value(lhs.span, "string_eq_lhs_reload", deferred_lhs)?;

            if matches!((lhs_v.ty, rhs_v.ty), (CgTy::String, CgTy::String)) {
                let l = self.expect_cg_pointer(lhs_v, "string equality lhs");
                let r = self.expect_cg_pointer(rhs_v, "string equality rhs");
                let fn_val = self.declare_runtime_string_equals();
                let call = self
                    .builder
                    .build_call(fn_val, &[l.into(), r.into()], "str_eq")?;
                let raw_result = self.expect_basic_value(call, "String.equals equality result");
                let eq_i64 = self.expect_int_value(raw_result, "String.equals equality result");
                let is_eq = self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "str_eq_bool",
                )?;
                let result = match op {
                    ast::BinaryOp::Eq => is_eq,
                    ast::BinaryOp::Ne => self.builder.build_not(is_eq, "str_ne_bool")?,
                    _ => unreachable!("filtered by caller"),
                };
                return Ok(CgValue::bool(result));
            }

            panic!("codegen_equality: typecheck gate accepted non-string rhs for string equality");
        }
        let rhs_v = self.codegen_expr(rhs)?;

        // Bool == Bool
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::Bool, CgTy::Bool)) {
            let l = lhs_v.as_bool().unwrap();
            let r = rhs_v.as_bool().unwrap();
            let pred = match op {
                ast::BinaryOp::Eq => IntPredicate::EQ,
                ast::BinaryOp::Ne => IntPredicate::NE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(self.builder.build_int_compare(
                pred,
                l,
                r,
                "icmp_bool",
            )?));
        }

        // T0107: String == String — call scoop_string_equals(a, b) -> i64 (1=equal, 0=not)
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::String, CgTy::String)) {
            let l = self.expect_cg_pointer(lhs_v, "string equality lhs");
            let r = self.expect_cg_pointer(rhs_v, "string equality rhs");
            let fn_val = self.declare_runtime_string_equals();
            let call = self
                .builder
                .build_call(fn_val, &[l.into(), r.into()], "str_eq")?;
            let raw_result = self.expect_basic_value(call, "String.equals equality result");
            let eq_i64 = self.expect_int_value(raw_result, "String.equals equality result");
            let is_eq = self.builder.build_int_compare(
                IntPredicate::NE,
                eq_i64,
                self.context.i64_type().const_zero(),
                "str_eq_bool",
            )?;
            let result = match op {
                ast::BinaryOp::Eq => is_eq,
                ast::BinaryOp::Ne => self.builder.build_not(is_eq, "str_ne_bool")?,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(result));
        }

        if let (Some((l_raw, l_ty)), Some((r_raw, r_ty))) = (lhs_v.as_float(), rhs_v.as_float()) {
            let float_ty = self
                .unify_float_cg_types(lhs, l_ty, rhs, r_ty)
                .unwrap_or_else(|| {
                    panic!("codegen_equality: typecheck gate accepted incompatible float operands")
                });
            let l = self.cast_float(l_raw, l_ty, float_ty)?;
            let r = self.cast_float(r_raw, r_ty, float_ty)?;
            let pred = match op {
                ast::BinaryOp::Eq => FloatPredicate::OEQ,
                ast::BinaryOp::Ne => FloatPredicate::UNE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(
                self.builder.build_float_compare(pred, l, r, "fcmp_eq")?,
            ));
        }

        // Int == Int（含 int literal 吸收）
        let (l_raw, l_ty) = self.expect_cg_int(lhs_v, "integer equality lhs");
        let (r_raw, r_ty) = self.expect_cg_int(rhs_v, "integer equality rhs");

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).unwrap_or_else(|| {
            panic!("codegen_equality: typecheck gate accepted incompatible integer operands")
        });

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match op {
            ast::BinaryOp::Eq => IntPredicate::EQ,
            ast::BinaryOp::Ne => IntPredicate::NE,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp_eq")?,
        ))
    }

    pub(in crate::llvm::codegen) fn codegen_bool_logic(
        &mut self,
        _span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_v = self.codegen_expr(lhs)?;
        let rhs_v = self.codegen_expr(rhs)?;
        let l = self.expect_cg_bool(lhs_v, "bool operator lhs");
        let r = self.expect_cg_bool(rhs_v, "bool operator rhs");

        let out = match op {
            ast::BinaryOp::LogAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::LogOr => self.builder.build_or(l, r, "or")?,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(out))
    }

    pub(in crate::llvm::codegen) fn coerce_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match (value.ty, target) {
            // T1612: Nothing (bottom type) coerces to any target type.
            // This only occurs on unreachable paths (after Raise.raise, etc.),
            // so we return a phantom default value of the target type.
            (CgTy::Never, _) => self.default_value(at, target),
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Unit, CgTy::Ref) => {
                // early stage：允许把 `Unit` 装箱到 `Any`。
                //
                // 说明：
                // - 当前阶段有一部分"语句位置"的表达式仍会被类型系统视为 `Any`（例如某些 `block/when`），
                //   因此后端需要支持 `Unit -> Any` 的值提升；
                // - v0 阶段 runtime type descriptor 仍是占位（NULL），这里只保证"可执行/可回归"。
                let boxed = self.codegen_box_unit_to_ref(at)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Bool, CgTy::Bool) => Ok(value),
            (CgTy::Bool, CgTy::Int(int_ty)) => {
                let v = self.expect_cg_bool(value, "Bool -> Int coercion");
                let out =
                    self.builder
                        .build_int_z_extend(v, self.int_type(int_ty), "bool_to_int")?;
                Ok(CgValue::int(out, int_ty))
            }
            (CgTy::Bool, CgTy::Ref) => {
                // early stage：允许把 `Bool` 装箱到 `Any`（与 `Int -> Any` 一致）。
                //
                // 注意：
                // - 当前阶段 runtime type descriptor 仍是占位（NULL），因此这里只保证"可执行/可回归"，
                //   不承诺后续 runtime type casts 的可观察语义；
                // - 为复用现有 box 形态，这里把 `Bool` 扩展为 word-sized 无符号整数后按 int box 存储。
                let v = self.expect_cg_bool(value, "Bool -> Ref boxing coercion");
                let word = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                };
                let widened =
                    self.builder
                        .build_int_z_extend(v, self.int_type(word), "box_bool_to_word")?;
                let boxed = self.codegen_box_int_to_ref(at, widened, word)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Float64, CgTy::Float64) | (CgTy::Float32, CgTy::Float32) => Ok(value),
            (CgTy::Float64, CgTy::Float32) | (CgTy::Float32, CgTy::Float64) => {
                let (v, from) = self.expect_cg_float(value, "Float scalar coercion");
                let out = self.cast_float(v, from, target)?;
                Ok(CgValue::float(out, target))
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                let (v, _) = self.expect_cg_int(value, "Int scalar coercion");
                if v.is_constant_int()
                    && let Some(bits) = self.int_literal_bits_from_source_span_if_present(at, to)?
                {
                    return Ok(CgValue::int(self.int_type(to).const_int(bits, false), to));
                }
                let out = self.cast_int(v, from, to)?;
                Ok(CgValue::int(out, to))
            }
            (CgTy::String, CgTy::String) => Ok(value),
            (CgTy::String, CgTy::Ref) => {
                let ptr = self.expect_cg_pointer(value, "String -> Ref coercion");

                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "str_to_ref",
                )?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(casted.into()),
                })
            }
            (CgTy::Ref, CgTy::String) => {
                let ptr = self.expect_cg_pointer(value, "Ref -> String coercion");
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_scoop_string_ptr_type(),
                    "ref_to_str",
                )?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(casted.into()),
                })
            }
            (CgTy::Ref, CgTy::Ref) => Ok(value),
            (CgTy::Int(_), CgTy::Ref) => {
                // T0817：值类型装箱到 `Any`（当前阶段先只支持整数族）。
                let (raw_int, from_ty) = self.expect_cg_int(value, "Int -> Ref boxing coercion");
                let boxed = self.codegen_box_int_to_ref(at, raw_int, from_ty)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Enum(enum_ty), CgTy::Ref) => {
                let raw = self.expect_cg_value(value, "Enum -> Ref boxing coercion");
                let boxed = self.codegen_box_enum_to_ref(at, enum_ty, raw)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Tuple(from), CgTy::Tuple(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) => {
                let from_llvm = self.llvm_basic_type_of(at, CgTy::Struct(from))?;
                let to_llvm = self.llvm_basic_type_of(at, CgTy::Struct(to))?;
                if from_llvm == to_llvm {
                    Ok(value)
                } else {
                    Err(LlvmEmitError::Frontend {
                        message: format!(
                            "unsupported value coercion from {:?} to {:?}",
                            CgTy::Struct(from),
                            CgTy::Struct(to)
                        ),
                    })
                }
            }
            (CgTy::Enum(from), CgTy::Enum(to)) if from == to => Ok(value),
            (CgTy::String, CgTy::Enum(target_enum))
            | (CgTy::Ref, CgTy::Enum(target_enum))
            | (CgTy::Enum(_), CgTy::Enum(target_enum)) => {
                if let Some(coerced) =
                    self.try_coerce_pointer_like_to_option_enum(at, value, target_enum)?
                {
                    Ok(coerced)
                } else {
                    panic!(
                        "coerce_value: typecheck gate accepted invalid pointer-like Option coercion"
                    )
                }
            }
            (from, to) => Err(LlvmEmitError::Frontend {
                message: format!("unsupported value coercion from {from:?} to {to:?}"),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn try_coerce_pointer_like_to_option_enum(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target_enum: MonoTypeId,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Option(target_inner)) =
            self.types.kind(target_enum.inner())
        else {
            return Ok(None);
        };

        if !matches!(
            self.cg_enum_layout(at, target_enum)?.repr,
            CgEnumRepr::Niche {
                storage: NicheStorage::Pointer,
                ..
            }
        ) {
            return Ok(None);
        }

        let target_inner_cg = self.cg_ty_of_type_id(*target_inner, "Option<T> inner type");

        let Some(raw) = value.value else {
            return Ok(None);
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Ok(None);
        };

        match (value.ty, target_inner_cg) {
            (CgTy::Ref, CgTy::Ref) | (CgTy::String, CgTy::String) | (CgTy::String, CgTy::Ref) => {}
            (CgTy::Enum(source_enum), CgTy::Ref | CgTy::String)
                if matches!(
                    self.cg_enum_layout(at, source_enum)?.repr,
                    CgEnumRepr::Niche {
                        storage: NicheStorage::Pointer,
                        ..
                    }
                ) => {}
            _ => return Ok(None),
        }

        let target_llvm_ty = self.llvm_basic_type_of(at, CgTy::Enum(target_enum))?;
        let BasicTypeEnum::PointerType(ptr_ty) = target_llvm_ty else {
            return Ok(None);
        };

        let casted = self
            .builder
            .build_pointer_cast(ptr, ptr_ty, "option_ptr_coerce")?;
        Ok(Some(CgValue {
            ty: CgTy::Enum(target_enum),
            value: Some(casted.into()),
        }))
    }
}
