//! Threading and task transport intrinsics lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_spawn(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(block_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn named arg (block)",
                at: span.into(),
            });
        };

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `thread.spawn` 的参数类型在 sysroot 中固定为 `() -> Unit`；
                // - 与 `sync.Once.run` 一致：为了在 early stage 稳定 codegen，这里从 `TypeStore` 中
                //   查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "thread.threadSpawn block fun type",
                        at: block_expr.span.into(),
                    },
                )?;
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let Some(block_raw) = block_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn block value",
                at: block_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(block_obj_i8) = block_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn block type",
                at: block_expr.span.into(),
            });
        };

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "thread_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "thread_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "thread_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "thread_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "thread_fn_raw")?
            .into_pointer_value();

        let start_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let start_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, start_fn_ptr_ty, "thread_fn_typed")?;

        let rt = self.declare_runtime_thread_spawn();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt,
            &[env_ptr.into(), start_fn_ptr.into()],
            "thread_spawn",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(thread_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(thread_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_join(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_thread_join();
        let _ =
            self.build_call_preserving_gc_local_roots(span, rt, &[recv_ptr.into()], "thread_join")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_sleep_millis(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.sleepMillis arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(ms_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.sleepMillis named arg",
                at: span.into(),
            });
        };

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let ms_v = self.codegen_expr_in_expected_context(ms_expr, Some(CgTy::Int(value_word)))?;
        let ms_v = self.coerce_value(ms_expr.span, ms_v, CgTy::Int(value_word))?;
        let (ms_raw, ms_from) = ms_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "thread.sleepMillis ms value",
            at: ms_expr.span.into(),
        })?;

        let ms_to = IntTy {
            bits: 64,
            signed: true,
        };
        let ms_i64 = self.cast_int(ms_raw, ms_from, ms_to)?;

        let rt = self.declare_runtime_thread_sleep_millis();
        let _ = self.build_call_preserving_gc_local_roots(
            span,
            rt,
            &[ms_i64.into()],
            "thread_sleep_millis",
        )?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_yield(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.yield arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_thread_yield();
        let _ = self.build_call_preserving_gc_local_roots(span, rt, &[], "thread_yield")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_current_id(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_thread_current_id();
        let call = self.builder.build_call(rt, &[], "thread_current_id")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId return type",
                at: span.into(),
            });
        };

        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_task_transport_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match fqn {
            "scoop.core.__task_transport_pack" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport pack arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport pack named arg",
                        at: span.into(),
                    });
                };

                let packed_ty = result_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task transport pack result type",
                    at: span.into(),
                })?;
                if !self.is_task_transport_tuple_ty(packed_ty) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport pack carrier type",
                        at: span.into(),
                    });
                }

                let value_ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
                let value_cg =
                    self.cg_ty_of(value_ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "task transport pack arg cg type",
                            at: expr.span.into(),
                        })?;
                let value = self.codegen_initializer_expr(expr, value_cg, value_ty)?;
                let value = self.coerce_value(expr.span, value, value_cg)?;
                let (word, gc_ref) = self.encode_effect_transport_value(expr.span, value)?;
                self.build_task_transport_tuple_value(span, packed_ty, word, gc_ref)
            }
            "scoop.core.__task_transport_unpack" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport unpack arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport unpack named arg",
                        at: span.into(),
                    });
                };

                let target_ty = result_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task transport unpack result type",
                    at: span.into(),
                })?;
                let target_cg =
                    self.cg_ty_of(target_ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "task transport unpack target cg type",
                            at: span.into(),
                        })?;

                let carrier_ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
                if !self.is_task_transport_tuple_ty(carrier_ty) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport unpack carrier type",
                        at: expr.span.into(),
                    });
                }
                let carrier_cg =
                    self.cg_ty_of(carrier_ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "task transport unpack carrier cg type",
                            at: expr.span.into(),
                        })?;
                let carrier = self.codegen_initializer_expr(expr, carrier_cg, carrier_ty)?;
                let carrier = self.coerce_value(expr.span, carrier, carrier_cg)?;
                let (word, gc_ref) = self.split_task_transport_tuple_value(expr.span, carrier)?;
                self.decode_effect_transport_value(span, word, gc_ref, target_cg)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot task transport intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match fqn {
            "scoop.core.__scoop_thread_spawn_join_resume_u64" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(k_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume named arg (continuation)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume named arg (value)",
                        at: span.into(),
                    });
                };

                let k_v = self.codegen_expr_in_expected_context(k_expr, Some(CgTy::Ref))?;
                let k_v = self.coerce_value(k_expr.span, k_v, CgTy::Ref)?;
                let Some(k_raw) = k_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume continuation value",
                        at: k_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(k_ptr) = k_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume continuation type",
                        at: k_expr.span.into(),
                    });
                };

                let value_v = self.codegen_expr(value_expr)?;
                let value_word = self.coerce_u64_word(value_expr.span, value_v)?;

                // runtime ABI：`void scoop_thread_spawn_join_resume_u64(void* k, uint64_t resume_value)`
                let rt = self.declare_runtime_thread_spawn_join_resume_u64();
                let k_i8 = self.builder.build_pointer_cast(
                    k_ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "thread_resume_k_i8",
                )?;
                let _ = self.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[k_i8.into(), value_word.into()],
                    "thread_spawn_join_resume",
                )?;
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot thread intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }
}
