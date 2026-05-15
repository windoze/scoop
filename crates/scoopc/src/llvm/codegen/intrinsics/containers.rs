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
}
