//! Container and array intrinsic lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_array_builder_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match fqn {
            "scoop.core.__scoop_array_builder_new" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_array_builder_new();
                let call = self.builder.build_call(rt, &[], "array_builder_new")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(ptr.into()),
                })
            }
            "scoop.core.__scoop_array_builder_push"
            | "scoop.core.__scoop_array_builder_push_string" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(builder_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push value named arg",
                        at: span.into(),
                    });
                };

                let builder_v =
                    self.codegen_expr_in_expected_context(builder_expr, Some(CgTy::Ref))?;
                let builder_v = self.coerce_value(builder_expr.span, builder_v, CgTy::Ref)?;
                let Some(builder_raw) = builder_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder value",
                        at: builder_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(builder_ptr) = builder_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder type",
                        at: builder_expr.span.into(),
                    });
                };
                let deferred_builder = self.defer_gc_ref_pointer(
                    builder_expr.span,
                    "array_builder_push_builder",
                    builder_ptr,
                )?;

                let value_v = self.codegen_expr(value_expr)?;
                match value_v.ty {
                    CgTy::Ref | CgTy::String => {
                        // ref/string 元素：保持为 `addrspace(1)` 指针，避免 ptr->u64 编码（为 statepoint/stackmap 做准备）。
                        let v = self.coerce_value(value_expr.span, value_v, CgTy::Ref)?;
                        let Some(raw) = v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "array_builder_push ref value",
                                at: value_expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "array_builder_push ref type",
                                at: value_expr.span.into(),
                            });
                        };

                        let builder_ptr = self.reload_deferred_gc_ref_without_clearing(
                            builder_expr.span,
                            "array_builder_push_builder_reload",
                            &deferred_builder,
                        )?;

                        let rt = self.declare_runtime_array_builder_push_ref();
                        let _ = self.builder.build_call(
                            rt,
                            &[builder_ptr.into(), ptr.into()],
                            "array_builder_push_ref",
                        )?;
                    }
                    _ => {
                        // word 元素：沿用旧 ABI（u64）。
                        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;
                        let builder_ptr = self.reload_deferred_gc_ref_without_clearing(
                            builder_expr.span,
                            "array_builder_push_builder_reload",
                            &deferred_builder,
                        )?;
                        let rt = self.declare_runtime_array_builder_push_u64();
                        let _ = self.builder.build_call(
                            rt,
                            &[builder_ptr.into(), word_u64.into()],
                            "array_builder_push_u64",
                        )?;
                    }
                }
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_array_builder_build_array"
            | "scoop.core.__scoop_array_builder_build_mutable_array"
            | "scoop.core.__scoop_array_builder_build_array_string" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(builder_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder named arg",
                        at: span.into(),
                    });
                };

                let builder_v =
                    self.codegen_expr_in_expected_context(builder_expr, Some(CgTy::Ref))?;
                let builder_v = self.coerce_value(builder_expr.span, builder_v, CgTy::Ref)?;
                let Some(builder_raw) = builder_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder value",
                        at: builder_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(builder_ptr) = builder_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder type",
                        at: builder_expr.span.into(),
                    });
                };
                let deferred_builder = self.defer_gc_ref_pointer(
                    builder_expr.span,
                    "array_builder_build_builder",
                    builder_ptr,
                )?;

                let rt = match fqn {
                    "scoop.core.__scoop_array_builder_build_array"
                    | "scoop.core.__scoop_array_builder_build_array_string" => {
                        self.declare_runtime_array_builder_build_array()
                    }
                    "scoop.core.__scoop_array_builder_build_mutable_array" => {
                        self.declare_runtime_array_builder_build_mutable_array()
                    }
                    _ => unreachable!("match arms cover all cases"),
                };

                let call = self.builder.build_call(
                    rt,
                    &[self
                        .reload_deferred_gc_ref_without_clearing(
                            builder_expr.span,
                            "array_builder_build_builder_reload",
                            &deferred_builder,
                        )?
                        .into()],
                    "array_builder_build",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(ptr.into()),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown array builder intrinsic",
                at: callee_span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_array_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let _gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        // helper：从 args[i] 取出位置参数 expr
        let positional = |idx: usize| -> Result<&hir::Expr, LlvmEmitError> {
            let Some(arg) = args.get(idx) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "array intrinsic missing arg",
                    at: span.into(),
                });
            };
            match arg {
                hir::CallArg::Positional(expr) => Ok(expr),
                hir::CallArg::Named { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "array intrinsic named arg",
                    at: span.into(),
                }),
            }
        };

        match fqn {
            "scoop.core.size" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let rt = self.declare_runtime_array_len();
                let call = self
                    .builder
                    .build_call(rt, &[arr_ptr.into()], "array_len")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(len_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size return type",
                        at: span.into(),
                    });
                };

                let len_word = self.cast_int(len_u64, from_u64, value_word)?;
                Ok(CgValue::int(len_word, value_word))
            }
            "scoop.core.get" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let idx_expr = positional(1)?;

                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let idx_v =
                    self.codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) =
                    idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get index value",
                        at: idx_expr.span.into(),
                    })?;
                let idx_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let idx_i64 = self.cast_int(idx_raw, idx_from, idx_to)?;

                let elem_ty = self
                    .infer_array_element_word_cg_ty(recv_expr)
                    .or_else(|| {
                        expected.filter(|ty| {
                            matches!(
                                ty,
                                CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                            )
                        })
                    })
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get element type",
                        at: callee_span.into(),
                    })?;

                match elem_ty {
                    CgTy::Ref | CgTy::String => {
                        let rt = self.declare_runtime_array_get_ref();
                        let call = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into()],
                            "array_get_ref",
                        )?;
                        let raw = call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return value",
                                at: span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return type",
                                at: span.into(),
                            });
                        };

                        match elem_ty {
                            CgTy::Ref => Ok(CgValue {
                                ty: CgTy::Ref,
                                value: Some(ptr.into()),
                            }),
                            CgTy::String => {
                                let str_ptr_ty = self.llvm_scoop_string_ptr_type();
                                let casted = self.builder.build_pointer_cast(
                                    ptr,
                                    str_ptr_ty,
                                    "ref_to_str",
                                )?;
                                Ok(CgValue {
                                    ty: CgTy::String,
                                    value: Some(casted.into()),
                                })
                            }
                            _ => unreachable!("match arms cover all pointer element types"),
                        }
                    }
                    _ => {
                        let rt = self.declare_runtime_array_get_u64();
                        let call = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into()],
                            "array_get_u64",
                        )?;
                        let raw = call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return value",
                                at: span.into(),
                            },
                        )?;
                        let BasicValueEnum::IntValue(word_u64) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return type",
                                at: span.into(),
                            });
                        };
                        self.decode_u64_word_to_cg_value(span, word_u64, elem_ty)
                    }
                }
            }
            "scoop.core.set" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let idx_expr = positional(1)?;
                let value_expr = positional(2)?;

                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let idx_v =
                    self.codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) =
                    idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set index value",
                        at: idx_expr.span.into(),
                    })?;
                let idx_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let idx_i64 = self.cast_int(idx_raw, idx_from, idx_to)?;

                // 尽量使用 receiver 的静态类型（type args）来决定 value 的 codegen/编码方式；
                // 若无法恢复，则退化为"按 value 表达式自身的 codegen 类型编码为 u64"。
                let elem_ty = self.infer_array_element_word_cg_ty(recv_expr);
                match elem_ty {
                    Some(CgTy::Ref) | Some(CgTy::String) => {
                        let expected_elem_ty = elem_ty.unwrap();
                        let v = self
                            .codegen_expr_in_expected_context(value_expr, Some(expected_elem_ty))?;
                        let v = self.coerce_value(value_expr.span, v, expected_elem_ty)?;
                        let v = self.coerce_value(value_expr.span, v, CgTy::Ref)?;
                        let Some(raw) = v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "MutableArray.set ref value",
                                at: value_expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "MutableArray.set ref type",
                                at: value_expr.span.into(),
                            });
                        };

                        let rt = self.declare_runtime_array_set_ref();
                        let _ = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into(), ptr.into()],
                            "array_set_ref",
                        )?;
                    }
                    _ => {
                        let value_v = match elem_ty {
                            Some(elem_ty) => {
                                let v = self
                                    .codegen_expr_in_expected_context(value_expr, Some(elem_ty))?;
                                self.coerce_value(value_expr.span, v, elem_ty)?
                            }
                            None => self.codegen_expr(value_expr)?,
                        };
                        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;

                        let rt = self.declare_runtime_array_set_u64();
                        let _ = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into(), word_u64.into()],
                            "array_set_u64",
                        )?;
                    }
                }
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown array intrinsic",
                at: callee_span.into(),
            }),
        }
    }

    fn infer_array_element_word_cg_ty(&self, receiver: &hir::Expr) -> Option<CgTy> {
        let receiver_ty = match &receiver.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                self.function_cx.env.get(*id)?.hir_ty?
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.top_level_value_ty(fqn)?
            }
            _ => return None,
        };

        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
        // codegen 侧需要把它们视为"array-like"，否则 `xs.get(i)` 在被 `print/println` 等
        // 以 `String` expected context 调用时，可能会错误地把元素解码为 `String`。
        if !matches!(
            nominal.fqn.as_str(),
            "scoop.core.Array"
                | "scoop.core.MutableArray"
                | "scoop.core.List"
                | "scoop.core.MutableList"
        ) {
            return None;
        }
        let elem_ty = *nominal.args.first()?;
        let cg = self.cg_ty_of(elem_ty)?;

        // 当前 runtime array 以 "u64 word buffer" 表示元素，因此这里限制为可编码为 u64 的类型。
        match cg {
            CgTy::Unit
            | CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref => Some(cg),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) | CgTy::Never => None,
        }
    }

    fn decode_u64_word_to_cg_value(
        &mut self,
        at: crate::span::Span,
        word_u64: IntValue<'ctx>,
        to: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        match to {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool => {
                let is_true = self.builder.build_int_compare(
                    IntPredicate::NE,
                    word_u64,
                    self.context.i64_type().const_zero(),
                    "u64_to_bool",
                )?;
                Ok(CgValue::bool(is_true))
            }
            CgTy::Float64 => {
                let raw = self
                    .builder
                    .build_bit_cast(word_u64, self.context.f64_type(), "u64_to_f64_bits")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float64))
            }
            CgTy::Float32 => {
                let bits32 = self.builder.build_int_truncate(
                    word_u64,
                    self.context.i32_type(),
                    "u64_to_f32_bits",
                )?;
                let raw = self
                    .builder
                    .build_bit_cast(bits32, self.context.f32_type(), "i32_to_f32_bits")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float32))
            }
            CgTy::Int(int_ty) => {
                let decoded = self.cast_int(word_u64, from_u64, int_ty)?;
                Ok(CgValue::int(decoded, int_ty))
            }
            CgTy::Ref | CgTy::String => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "decode u64 word to gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "decode u64 word to composite value",
                    at: at.into(),
                })
            }
        }
    }
}
