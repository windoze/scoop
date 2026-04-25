//! Atomic integer intrinsics lowering.

use inkwell::AtomicOrdering;

use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicIntLvalueMode {
    ReadOnly,
    ReadWrite,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_atomic_int_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let atomic_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        match fqn {
            "scoop.unsafe.__atomicIntLoad" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad named arg",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadOnly,
                )?;

                let llvm_ty = self.int_type(atomic_word);
                let loaded = self.builder.build_load(llvm_ty, ptr, "atomic_int_load")?;
                let inst =
                    loaded
                        .as_instruction_value()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntLoad load instruction",
                            at: target_expr.span.into(),
                        })?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad set ordering",
                        at: target_expr.span.into(),
                    })?;

                let BasicValueEnum::IntValue(raw) = loaded else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad return type",
                        at: target_expr.span.into(),
                    });
                };
                Ok(CgValue::int(raw, atomic_word))
            }
            "scoop.unsafe.__atomicIntStore" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore named arg (value)",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadWrite,
                )?;

                let v = self
                    .codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(atomic_word)))?;
                let v = self.coerce_value(value_expr.span, v, CgTy::Int(atomic_word))?;
                let (raw_int, from) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicIntStore value",
                    at: value_expr.span.into(),
                })?;
                let raw_int = self.cast_int(raw_int, from, atomic_word)?;

                let inst = self.builder.build_store(ptr, raw_int)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore set ordering",
                        at: target_expr.span.into(),
                    })?;
                Ok(CgValue::unit())
            }
            "scoop.unsafe.__atomicIntCompareExchange" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(expected_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (expected)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(desired_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (desired)",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadWrite,
                )?;

                let expected_v = self.codegen_expr_in_expected_context(
                    expected_expr,
                    Some(CgTy::Int(atomic_word)),
                )?;
                let expected_v =
                    self.coerce_value(expected_expr.span, expected_v, CgTy::Int(atomic_word))?;
                let (expected_raw, expected_from) =
                    expected_v
                        .as_int()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntCompareExchange expected",
                            at: expected_expr.span.into(),
                        })?;
                let expected_raw = self.cast_int(expected_raw, expected_from, atomic_word)?;

                let desired_v = self
                    .codegen_expr_in_expected_context(desired_expr, Some(CgTy::Int(atomic_word)))?;
                let desired_v =
                    self.coerce_value(desired_expr.span, desired_v, CgTy::Int(atomic_word))?;
                let (desired_raw, desired_from) =
                    desired_v
                        .as_int()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntCompareExchange desired",
                            at: desired_expr.span.into(),
                        })?;
                let desired_raw = self.cast_int(desired_raw, desired_from, atomic_word)?;

                // LLVM: `cmpxchg ptr, expected, desired` returns `{ T, i1 }`.
                let cx = self.builder.build_cmpxchg(
                    ptr,
                    expected_raw,
                    desired_raw,
                    AtomicOrdering::SequentiallyConsistent,
                    AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self.builder.build_extract_value(cx, 1, "cmpxchg_success")?;
                let BasicValueEnum::IntValue(ok) = success else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange success type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::bool(ok))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot atomicInt intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_atomic_int_lvalue_ptr(
        &mut self,
        at: crate::span::Span,
        target_expr: &hir::Expr,
        mode: AtomicIntLvalueMode,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let expected = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let place = self.codegen_addressable_place(target_expr)?;

        if mode == AtomicIntLvalueMode::ReadWrite && !place.writable {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt requires mutable lvalue",
                at: at.into(),
            });
        }

        let CgTy::Int(int_ty) = place.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt target type",
                at: at.into(),
            });
        };
        if int_ty != expected {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt target width",
                at: at.into(),
            });
        }

        Ok(place.ptr)
    }
}
