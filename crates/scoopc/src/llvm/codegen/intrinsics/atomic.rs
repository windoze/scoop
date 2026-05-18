//! Atomic integer intrinsics lowering.

use inkwell::AtomicOrdering;

use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicIntLvalueMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicRefLvalueMode {
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

                let deferred_class_place =
                    if let hir::ExprKind::MemberAccess { receiver, member } = &target_expr.kind {
                        if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() {
                            let receiver_hir_ty = self
                                .resolve_expr_concrete_type(receiver)
                                .unwrap_or(receiver.ty);
                            self.defer_class_field_place(
                                receiver,
                                member.span,
                                fqn,
                                receiver_hir_ty,
                                "atomic_int_store",
                            )?
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                let fallback_ptr = if let Some(place) = deferred_class_place.as_ref() {
                    if !place.writable {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt requires mutable lvalue",
                            at: target_expr.span.into(),
                        });
                    }
                    let CgTy::Int(int_ty) = place.field_cg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt target type",
                            at: target_expr.span.into(),
                        });
                    };
                    if int_ty != atomic_word {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt target width",
                            at: target_expr.span.into(),
                        });
                    }
                    None
                } else {
                    Some(self.codegen_atomic_int_lvalue_ptr(
                        target_expr.span,
                        target_expr,
                        AtomicIntLvalueMode::ReadWrite,
                    )?)
                };

                let v = self
                    .codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(atomic_word)))?;
                let v = self.coerce_value(value_expr.span, v, CgTy::Int(atomic_word))?;
                let (raw_int, from) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicIntStore value",
                    at: value_expr.span.into(),
                })?;
                let raw_int = self.cast_int(raw_int, from, atomic_word)?;

                let ptr = if let Some(place) = deferred_class_place.as_ref() {
                    self.reload_deferred_class_field_place_ptr(
                        target_expr.span,
                        place,
                        "atomic_int_store",
                    )?
                } else {
                    fallback_ptr.expect("non-class atomic store pointer")
                };

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

                let deferred_class_place =
                    if let hir::ExprKind::MemberAccess { receiver, member } = &target_expr.kind {
                        if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() {
                            let receiver_hir_ty = self
                                .resolve_expr_concrete_type(receiver)
                                .unwrap_or(receiver.ty);
                            self.defer_class_field_place(
                                receiver,
                                member.span,
                                fqn,
                                receiver_hir_ty,
                                "atomic_int_cmpxchg",
                            )?
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                let fallback_ptr = if let Some(place) = deferred_class_place.as_ref() {
                    if !place.writable {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt requires mutable lvalue",
                            at: target_expr.span.into(),
                        });
                    }
                    let CgTy::Int(int_ty) = place.field_cg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt target type",
                            at: target_expr.span.into(),
                        });
                    };
                    if int_ty != atomic_word {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt target width",
                            at: target_expr.span.into(),
                        });
                    }
                    None
                } else {
                    Some(self.codegen_atomic_int_lvalue_ptr(
                        target_expr.span,
                        target_expr,
                        AtomicIntLvalueMode::ReadWrite,
                    )?)
                };

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

                let ptr = if let Some(place) = deferred_class_place.as_ref() {
                    self.reload_deferred_class_field_place_ptr(
                        target_expr.span,
                        place,
                        "atomic_int_cmpxchg",
                    )?
                } else {
                    fallback_ptr.expect("non-class atomic cmpxchg pointer")
                };

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

    pub(in crate::llvm::codegen) fn codegen_sysroot_atomic_ref_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match atomic_intrinsic_base_fqn(fqn) {
            "scoop.unsafe.__atomicRefLoad" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefLoad arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefLoad named arg",
                        at: span.into(),
                    });
                };

                let place = self.codegen_atomic_ref_lvalue_place(
                    target_expr.span,
                    target_expr,
                    AtomicRefLvalueMode::ReadOnly,
                )?;
                let storage_ty = self.atomic_ref_storage_ty(target_expr.span, place.ty)?;
                let llvm_ty = self.llvm_basic_type_of(target_expr.span, storage_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, place.ptr, "atomic_ref_load")?;
                let inst =
                    loaded
                        .as_instruction_value()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicRefLoad load instruction",
                            at: target_expr.span.into(),
                        })?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefLoad set ordering",
                        at: target_expr.span.into(),
                    })?;

                Ok(CgValue {
                    ty: storage_ty,
                    value: Some(loaded),
                })
            }
            "scoop.unsafe.__atomicRefStore" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore named arg (value)",
                        at: span.into(),
                    });
                };

                let deferred_class_place =
                    self.defer_atomic_ref_class_field_place(target_expr, "atomic_ref_store")?;
                let fallback_place = if let Some(place) = deferred_class_place.as_ref() {
                    if !place.writable {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicRef requires mutable lvalue",
                            at: target_expr.span.into(),
                        });
                    }
                    self.atomic_ref_storage_ty(target_expr.span, place.field_cg)?;
                    None
                } else {
                    Some(self.codegen_atomic_ref_lvalue_place(
                        target_expr.span,
                        target_expr,
                        AtomicRefLvalueMode::ReadWrite,
                    )?)
                };
                let storage_ty = deferred_class_place
                    .as_ref()
                    .map(|place| place.field_cg)
                    .or_else(|| fallback_place.as_ref().map(|place| place.ty))
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRef target place",
                        at: target_expr.span.into(),
                    })?;
                let storage_ty = self.atomic_ref_storage_ty(target_expr.span, storage_ty)?;

                let raw =
                    self.codegen_atomic_ref_operand(value_expr, storage_ty, "atomicRefStore")?;
                let ptr = if let Some(place) = deferred_class_place.as_ref() {
                    self.reload_deferred_class_field_place_ptr(
                        target_expr.span,
                        place,
                        "atomic_ref_store",
                    )?
                } else {
                    fallback_place
                        .expect("non-class atomic ref store pointer")
                        .ptr
                };

                let inst = self.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore set ordering",
                        at: target_expr.span.into(),
                    })?;
                self.promote_gc_pointer_with_write_barrier(value_expr.span, raw)?;
                Ok(CgValue::unit())
            }
            "scoop.unsafe.__atomicRefCompareExchange" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(expected_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange named arg (expected)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(desired_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange named arg (desired)",
                        at: span.into(),
                    });
                };

                let deferred_class_place =
                    self.defer_atomic_ref_class_field_place(target_expr, "atomic_ref_cmpxchg")?;
                let fallback_place = if let Some(place) = deferred_class_place.as_ref() {
                    if !place.writable {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicRef requires mutable lvalue",
                            at: target_expr.span.into(),
                        });
                    }
                    self.atomic_ref_storage_ty(target_expr.span, place.field_cg)?;
                    None
                } else {
                    Some(self.codegen_atomic_ref_lvalue_place(
                        target_expr.span,
                        target_expr,
                        AtomicRefLvalueMode::ReadWrite,
                    )?)
                };
                let storage_ty = deferred_class_place
                    .as_ref()
                    .map(|place| place.field_cg)
                    .or_else(|| fallback_place.as_ref().map(|place| place.ty))
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRef target place",
                        at: target_expr.span.into(),
                    })?;
                let storage_ty = self.atomic_ref_storage_ty(target_expr.span, storage_ty)?;

                let expected = self.codegen_atomic_ref_operand(
                    expected_expr,
                    storage_ty,
                    "atomicRefCompareExchange expected",
                )?;
                let desired = self.codegen_atomic_ref_operand(
                    desired_expr,
                    storage_ty,
                    "atomicRefCompareExchange desired",
                )?;
                let ptr = if let Some(place) = deferred_class_place.as_ref() {
                    self.reload_deferred_class_field_place_ptr(
                        target_expr.span,
                        place,
                        "atomic_ref_cmpxchg",
                    )?
                } else {
                    fallback_place
                        .expect("non-class atomic ref cmpxchg pointer")
                        .ptr
                };

                let cx = self.builder.build_cmpxchg(
                    ptr,
                    expected,
                    desired,
                    AtomicOrdering::SequentiallyConsistent,
                    AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self.builder.build_extract_value(cx, 1, "cmpxchg_success")?;
                let BasicValueEnum::IntValue(ok) = success else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange success type",
                        at: span.into(),
                    });
                };
                self.codegen_atomic_ref_cas_barrier(desired_expr.span, ok, desired)?;
                Ok(CgValue::bool(ok))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot atomicRef intrinsic callee",
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

    fn defer_atomic_ref_class_field_place(
        &mut self,
        target_expr: &hir::Expr,
        name_prefix: &str,
    ) -> Result<Option<DeferredClassFieldPlace<'ctx>>, LlvmEmitError> {
        let hir::ExprKind::MemberAccess { receiver, member } = &target_expr.kind else {
            return Ok(None);
        };
        let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
            return Ok(None);
        };
        let receiver_hir_ty = self
            .resolve_expr_concrete_type(receiver)
            .unwrap_or(receiver.ty);
        self.defer_class_field_place(receiver, member.span, fqn, receiver_hir_ty, name_prefix)
    }

    fn codegen_atomic_ref_lvalue_place(
        &mut self,
        at: crate::span::Span,
        target_expr: &hir::Expr,
        mode: AtomicRefLvalueMode,
    ) -> Result<AddressablePlace<'ctx>, LlvmEmitError> {
        let place = self.codegen_addressable_place(target_expr)?;
        if mode == AtomicRefLvalueMode::ReadWrite && !place.writable {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicRef requires mutable lvalue",
                at: at.into(),
            });
        }
        self.atomic_ref_storage_ty(at, place.ty)?;
        Ok(place)
    }

    fn atomic_ref_storage_ty(
        &self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<CgTy, LlvmEmitError> {
        if matches!(ty, CgTy::Ref | CgTy::String) {
            return Ok(ty);
        }
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "atomicRef target type",
            at: at.into(),
        })
    }

    fn codegen_atomic_ref_operand(
        &mut self,
        expr: &hir::Expr,
        storage_ty: CgTy,
        kind: &'static str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.codegen_expr_in_expected_context(expr, Some(storage_ty))?;
        let value = self.coerce_value(expr.span, value, storage_ty)?;
        let Some(raw) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: expr.span.into(),
            });
        };
        Ok(ptr)
    }

    fn codegen_atomic_ref_cas_barrier(
        &mut self,
        span: crate::span::Span,
        success: IntValue<'ctx>,
        desired: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.expect_current_function("atomicRefCompareExchange barrier");
        let barrier_bb = self
            .context
            .append_basic_block(function, "atomic_ref_cas_barrier");
        let cont_bb = self
            .context
            .append_basic_block(function, "atomic_ref_cas_cont");
        self.builder
            .build_conditional_branch(success, barrier_bb, cont_bb)?;

        self.builder.position_at_end(barrier_bb);
        self.promote_gc_pointer_with_write_barrier(span, desired)?;
        self.builder.build_unconditional_branch(cont_bb)?;

        self.builder.position_at_end(cont_bb);
        Ok(())
    }
}

fn atomic_intrinsic_base_fqn(fqn: &str) -> &str {
    fqn.split("::<")
        .next()
        .unwrap_or(fqn)
        .split("$overload")
        .next()
        .unwrap_or(fqn)
}
