//! GC/statepoint codegen（T0102e：从 `codegen/mod.rs` 拆分）。

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn try_codegen_sysroot_gc_debug_intrinsics(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        // TODO T0910：GC v0（mark-sweep，测试辅助）。
        if fqn == "scoop.core.__scoop_gc_collect" {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc collect arity mismatch",
                    at: span.into(),
                });
            }

            // 重要：该调用点必须能产出 stackmap record，否则 GC 期间无法枚举 managed roots。
            let rt = self.declare_runtime_gc_collect_safepoint();
            let _ =
                self.build_call_preserving_gc_local_roots(span, rt, &[], "gc_collect_safepoint")?;
            return Ok(Some(CgValue::unit()));
        }

        if fqn == "scoop.core.__scoop_gc_debug_heap_object_count" {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc heap object count arity mismatch",
                    at: span.into(),
                });
            }

            let rt = self.declare_runtime_gc_debug_heap_object_count();
            let call = self
                .builder
                .build_call(rt, &[], "gc_debug_heap_object_count")?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc heap object count return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(raw_int) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc heap object count return type",
                    at: span.into(),
                });
            };

            let from = IntTy {
                bits: 64,
                signed: false,
            };
            let to = IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            };
            let casted = self.cast_int(raw_int, from, to)?;
            return Ok(Some(CgValue::int(casted, to)));
        }

        if fqn == "scoop.core.__scoop_gc_debug_alloc_garbage" {
            if args.len() != 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc debug alloc garbage arity mismatch",
                    at: span.into(),
                });
            }

            let hir::CallArg::Positional(count_expr) = &args[0] else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc debug alloc garbage named arg",
                    at: span.into(),
                });
            };

            let value_word = IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            };

            let count_v =
                self.codegen_expr_in_expected_context(count_expr, Some(CgTy::Int(value_word)))?;
            let count_v = self.coerce_value(count_expr.span, count_v, CgTy::Int(value_word))?;
            let (count_raw, count_from) =
                count_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "gc debug alloc garbage count value",
                    at: count_expr.span.into(),
                })?;
            let count_to = IntTy {
                bits: 64,
                signed: true,
            };
            let count_i64 = self.cast_int(count_raw, count_from, count_to)?;

            let rt = self.declare_runtime_gc_debug_alloc_garbage();
            let _ = self.build_call_preserving_gc_local_roots(
                span,
                rt,
                &[count_i64.into()],
                "gc_debug_alloc_garbage",
            )?;
            return Ok(Some(CgValue::unit()));
        }

        if fqn == "scoop.core.__scoop_stackmap_statepoint_smoke" {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "stackmap statepoint smoke arity mismatch",
                    at: span.into(),
                });
            }

            // 重要：
            // - 默认 explicit mode 不再给托管函数统一打 `gc "statepoint-example"`；
            // - 该 helper 是保留下来的“显式 opt-in stackmap smoke”边界，因此需要仅对当前函数
            //   恢复 LLVM statepoint GC strategy，让调用点重新产出真实 statepoint/stackmap record；
            // - 这里只给显式调用该 helper 的函数开启该策略，避免把默认 explicit-root-frame 路线
            //   再次退回到隐式 stackmap 依赖。
            let current_fun = self
                .builder
                .get_insert_block()
                .and_then(|bb| bb.get_parent())
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "stackmap statepoint smoke caller function",
                    at: span.into(),
                })?;
            current_fun.set_gc("statepoint-example");

            // 该 helper 仍必须经 ordinary managed runtime call 进入 IR；不能走 `@Extern` 的
            // native/leaf lowering，否则调用点本身不会留下 stackmap record。
            let rt = self.declare_runtime_stackmap_statepoint_smoke();
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt,
                &[],
                "stackmap_statepoint_smoke",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "stackmap statepoint smoke return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(raw_int) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "stackmap statepoint smoke return type",
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
            let casted = self.cast_int(raw_int, from, to)?;
            return Ok(Some(CgValue::int(casted, to)));
        }

        Ok(None)
    }

    pub(super) fn codegen_sysroot_gc_pin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(obj_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin named arg",
                at: span.into(),
            });
        };

        let Some(CgTy::Struct(pinned_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin call without expected pinned type",
                at: callee_span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", callee_span)?;

        let obj_v = self.codegen_expr_in_expected_context(obj_expr, Some(field_cg_ty))?;
        let obj_v = self.coerce_value(obj_expr.span, obj_v, field_cg_ty)?;

        // 运行期 pin 需要 `void*`：统一使用 `i8*`。
        let obj_ref = self.coerce_value(obj_expr.span, obj_v, CgTy::Ref)?;
        let Some(obj_raw) = obj_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg value",
                at: obj_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = obj_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg type",
                at: obj_expr.span.into(),
            });
        };

        let rt_pin = self.declare_runtime_gc_pin();
        let call = self
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "gc_pin")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_pin_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_pin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_pin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_pin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let llvm_struct_ty = self.llvm_struct_type(span, pinned_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        let raw_field: BasicValueEnum<'ctx> = match field_cg_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            _ => obj_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin field value",
                at: obj_expr.span.into(),
            })?,
        };
        agg = self
            .builder
            .build_insert_value(agg, raw_field, field_idx, "pinned_value")?;
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(pinned_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_new(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(obj_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew named arg",
                at: span.into(),
            });
        };

        let Some(CgTy::Struct(handle_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew call without expected handle type",
                at: callee_span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", callee_span)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew raw field type",
                at: callee_span.into(),
            });
        };

        let obj_v = self.codegen_expr_in_expected_context(obj_expr, Some(CgTy::Ref))?;
        let obj_ref = self.coerce_value(obj_expr.span, obj_v, CgTy::Ref)?;
        let Some(obj_raw) = obj_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew arg value",
                at: obj_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = obj_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew arg type",
                at: obj_expr.span.into(),
            });
        };

        let rt_handle_new = self.declare_runtime_gc_handle_new();
        let call = self
            .builder
            .build_call(rt_handle_new, &[obj_ptr.into()], "gc_handle_new")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(handle_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleNew return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            handle_i64,
            self.context.i64_type().const_zero(),
            "gc_handle_new_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_handle_new_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_new_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_new_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let from = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_word = self.cast_int(handle_i64, from, field_int_ty)?;
        let llvm_struct_ty = self.llvm_struct_type(span, handle_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        agg = self.builder.build_insert_value(
            agg,
            handle_word.as_basic_value_enum(),
            field_idx,
            "gc_handle_raw",
        )?;
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(handle_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_get(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(handle_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet named arg",
                at: span.into(),
            });
        };

        let handle_v = self.codegen_expr(handle_expr)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet arg type",
                at: handle_expr.span.into(),
            });
        };
        let Some(raw) = handle_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet arg value",
                at: handle_expr.span.into(),
            });
        };
        let BasicValueEnum::StructValue(struct_v) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet arg value type",
                at: handle_expr.span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", handle_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(handle_expr.span, field_cg_ty, extracted)?;

        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet raw field type",
                at: handle_expr.span.into(),
            });
        };
        let Some(field_raw) = field_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet raw value",
                at: handle_expr.span.into(),
            });
        };
        let BasicValueEnum::IntValue(handle_word) = field_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet raw value type",
                at: handle_expr.span.into(),
            });
        };

        let to_i64 = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_i64 = self.cast_int(handle_word, field_int_ty, to_i64)?;

        let rt_handle_get = self.declare_runtime_gc_handle_get();
        let call = self
            .builder
            .build_call(rt_handle_get, &[handle_i64.into()], "gc_handle_get")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleGet return type",
                at: span.into(),
            });
        };

        let obj_is_null = self
            .builder
            .build_is_null(obj_ptr, "gc_handle_get_is_null")?;
        let ok_cond = self.builder.build_not(obj_is_null, "gc_handle_get_ok")?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_handle_get_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_get_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_get_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_ptr.into()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_drop(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(handle_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop named arg",
                at: span.into(),
            });
        };

        let handle_v = self.codegen_expr(handle_expr)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop arg type",
                at: handle_expr.span.into(),
            });
        };
        let Some(raw) = handle_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop arg value",
                at: handle_expr.span.into(),
            });
        };
        let BasicValueEnum::StructValue(struct_v) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop arg value type",
                at: handle_expr.span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", handle_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(handle_expr.span, field_cg_ty, extracted)?;

        let CgTy::Int(field_int_ty) = field_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop raw field type",
                at: handle_expr.span.into(),
            });
        };
        let Some(field_raw) = field_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop raw value",
                at: handle_expr.span.into(),
            });
        };
        let BasicValueEnum::IntValue(handle_word) = field_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop raw value type",
                at: handle_expr.span.into(),
            });
        };

        let to_i64 = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_i64 = self.cast_int(handle_word, field_int_ty, to_i64)?;

        let rt_handle_drop = self.declare_runtime_gc_handle_drop();
        let call =
            self.builder
                .build_call(rt_handle_drop, &[handle_i64.into()], "gc_handle_drop")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.handleDrop return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_handle_drop_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    pub(super) fn codegen_sysroot_gc_unpin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(pinned_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin named arg",
                at: span.into(),
            });
        };

        let pinned_v = self.codegen_expr(pinned_expr)?;
        let CgTy::Struct(pinned_ty) = pinned_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg type",
                at: pinned_expr.span.into(),
            });
        };
        let Some(raw) = pinned_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg value",
                at: pinned_expr.span.into(),
            });
        };
        let BasicValueEnum::StructValue(struct_v) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg value type",
                at: pinned_expr.span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", pinned_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "pinned_value")?;
        let field_v = self.cg_value_from_loaded(pinned_expr.span, field_cg_ty, extracted)?;
        let field_ref = self.coerce_value(pinned_expr.span, field_v, CgTy::Ref)?;

        let Some(field_raw) = field_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin value",
                at: pinned_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = field_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin value type",
                at: pinned_expr.span.into(),
            });
        };

        let rt_unpin = self.declare_runtime_gc_unpin();
        let call = self
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "gc_unpin")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_unpin_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_unpin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_unpin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_unpin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    pub(super) fn gc_address_space(&self) -> AddressSpace {
        AddressSpace::from(GC_ADDRSPACE)
    }

    pub(super) fn llvm_ptr_type(&self, address_space: AddressSpace) -> PointerType<'ctx> {
        self.context.ptr_type(address_space)
    }

    pub(super) fn llvm_ptr_sized_int_type(
        &self,
        address_space: Option<AddressSpace>,
    ) -> IntType<'ctx> {
        self.context
            .ptr_sized_int_type(self.target_data, address_space)
    }

    /// LLVM addrspace(0)：native/unsafe 指针（C ABI / malloc buffer 等）。
    pub(super) fn llvm_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.llvm_ptr_type(AddressSpace::default())
    }

    /// LLVM addrspace(1)：GC-managed 引用指针（Any/class/interface/closure/...）。
    pub(super) fn llvm_gc_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.llvm_ptr_type(self.gc_address_space())
    }

    pub(super) fn llvm_scoop_string_type(&self) -> StructType<'ctx> {
        // 说明：该类型名用于 LLVM module 内部复用，不应与用户类型冲突（使用 runtime 命名空间前缀）。
        const TY_NAME: &str = "scoop.runtime.ScoopString";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { ScoopGcObjectHeader hdr; uint64_t len; const uint8_t *data; } ScoopString;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let len_ty = self.context.i64_type();
        let data_ty = self.llvm_i8_ptr_type();
        ty.set_body(&[header_ty.into(), len_ty.into(), data_ty.into()], false);
        ty
    }

    pub(super) fn llvm_scoop_string_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.llvm_ptr_type(self.gc_address_space())
    }

    fn local_gc_root_value_ptr_type(
        &mut self,
        at: crate::span::Span,
        local: &CgLocal<'ctx>,
    ) -> Result<Option<PointerType<'ctx>>, LlvmEmitError> {
        let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
        let BasicTypeEnum::PointerType(ptr_ty) = llvm_ty else {
            return Ok(None);
        };

        if ptr_ty.get_address_space() == self.gc_address_space() {
            Ok(Some(ptr_ty))
        } else {
            Ok(None)
        }
    }

    /// 收集当前 env 中“底层 LLVM 表示就是 GC 指针”的局部槽位。
    ///
    /// 说明：
    /// - 不能只按 `CgTy::Ref | CgTy::String` 判断；
    /// - `Option<Ref>` / `Option<Continuation>` 等 niche enum 也可能直接降成 `ptr addrspace(1)`；
    /// - statepoint 只会追踪 live SSA roots，不会自动把 `alloca ptr addrspace(1)` 当作根，
    ///   因此 ordinary 函数里的这类局部必须在 safepoint 前显式 load 成 SSA 值并在之后写回。
    pub(super) fn collect_conservative_gc_root_slots(
        &mut self,
        at: crate::span::Span,
    ) -> Result<
        Vec<(
            u32,
            PointerValue<'ctx>,
            PointerType<'ctx>,
            PointerValue<'ctx>,
        )>,
        LlvmEmitError,
    > {
        let mut locals = Vec::new();
        for frame in &self.function_cx.env.scopes {
            for (id, local) in frame {
                locals.push((id.as_u32(), *local));
            }
        }

        let mut slots = Vec::new();
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        for (local_id, local) in locals {
            if let Some(value_ptr_ty) = self.local_gc_root_value_ptr_type(at, &local)? {
                let slot_ptr =
                    self.local_ptr_for_use(at, local, &format!("gc_root_slot_{local_id}"))?;
                let needs_spill = self.conservative_gc_root_slot_needs_spill_writeback(slot_ptr);
                let frame_slot = if explicit_frame_enabled && needs_spill {
                    self.explicit_frame_slot_mirrors_for(local.ptr)
                        .and_then(|slots| slots.first().copied())
                        .map(|slot| {
                            self.rematerialize_ptr_in_current_block(
                                at,
                                slot,
                                &format!("explicit_gc_root_slot_{local_id}"),
                            )
                        })
                        .transpose()?
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "local explicit frame root slot",
                            at: at.into(),
                        })?
                } else {
                    slot_ptr
                };
                slots.push((local_id, slot_ptr, value_ptr_ty, frame_slot));
            }
        }
        for (index, extra) in self
            .function_cx
            .tracked_gc_root_slots
            .clone()
            .into_iter()
            .enumerate()
        {
            let slot = self.rematerialize_ptr_in_current_block(
                at,
                extra.slot,
                &format!("tracked_gc_root_slot_{index}"),
            )?;
            let frame_slot = if explicit_frame_enabled
                && self.conservative_gc_root_slot_needs_spill_writeback(slot)
            {
                self.rematerialize_ptr_in_current_block(
                    at,
                    extra.frame_slot,
                    &format!("tracked_explicit_gc_root_slot_{index}"),
                )?
            } else {
                slot
            };
            slots.push((
                u32::MAX - index as u32,
                slot,
                extra.value_ptr_ty,
                frame_slot,
            ));
        }
        slots.sort_by_key(|(id, _, _, _)| *id);
        Ok(slots)
    }

    fn conservative_gc_root_slot_needs_spill_writeback(&self, slot: PointerValue<'ctx>) -> bool {
        slot.get_type().get_address_space() == AddressSpace::default()
    }

    /// 在一次 ordinary safepoint 前后保守 keepalive 所有“需要手动 spill/writeback”的
    /// pointer-shaped GC locals。
    ///
    /// 做法：
    /// - 仅对 stack/native-slot 一类的 stack-backed 槽位执行 `load -> gc-live -> writeback`；
    /// - heap-backed 槽位（例如 effect frame / GC object field）本身已位于 traced heap 中，
    ///   运行时会直接更新真实槽位，不能再把“调用前旧 keepalive”写回覆盖新值；
    /// - 对 stack-backed 槽位，调用前先从局部槽位 load 出 SSA root，调用后再把
    ///   relocate 后的 SSA root store 回原槽位；
    /// - 这样 `rewrite-statepoints-for-gc` 才会把这些 stack locals 纳入 `gc-live` / stackmap。
    pub(super) fn with_conservative_gc_local_root_spills<T, F>(
        &mut self,
        at: crate::span::Span,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        let spills = self
            .collect_conservative_gc_root_slots(at)?
            .into_iter()
            .filter(|(_, slot, _, _)| self.conservative_gc_root_slot_needs_spill_writeback(*slot))
            .map(|(local_id, slot, value_ptr_ty, frame_slot)| {
                let source_slot = if explicit_frame_enabled {
                    frame_slot
                } else {
                    slot
                };
                let loaded = self
                    .builder
                    .build_load(
                        value_ptr_ty,
                        source_slot,
                        &format!("gc_root_keepalive_{local_id}"),
                    )?
                    .into_pointer_value();
                Ok((slot, frame_slot, loaded, value_ptr_ty))
            })
            .collect::<Result<Vec<_>, LlvmEmitError>>()?;

        let result = f(self)?;

        let Some(insert_block) = self.builder.get_insert_block() else {
            return Ok(result);
        };
        if let Some(term) = insert_block.get_terminator() {
            let builder = self.context.create_builder();
            builder.position_before(&term);
            for (slot, frame_slot, value, value_ptr_ty) in spills {
                if explicit_frame_enabled {
                    let reloaded = builder
                        .build_load(value_ptr_ty, frame_slot, "gc_root_keepalive_reload")?
                        .into_pointer_value();
                    let _ = builder.build_store(slot, reloaded)?;
                } else {
                    let _ = builder.build_store(slot, value)?;
                }
            }
            return Ok(result);
        }

        for (slot, frame_slot, value, value_ptr_ty) in spills {
            if explicit_frame_enabled {
                let reloaded = self
                    .builder
                    .build_load(value_ptr_ty, frame_slot, "gc_root_keepalive_reload")?
                    .into_pointer_value();
                let _ = self.builder.build_store(slot, reloaded)?;
            } else {
                let _ = self.builder.build_store(slot, value)?;
            }
        }

        Ok(result)
    }

    pub(super) fn build_call_preserving_gc_local_roots(
        &mut self,
        at: crate::span::Span,
        callee: FunctionValue<'ctx>,
        args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
        name: &str,
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        self.with_conservative_gc_local_root_spills(at, |cg| {
            Ok(cg.builder.build_call(callee, args, name)?)
        })
    }

    pub(super) fn llvm_gc_object_header_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_gc.h` 的 `ScoopGcObjectHeader`（TODO T0908）；
        // - 当前阶段用 `i8*` 作为 `next` 与 `type_desc` 的承载类型（不暴露具体指针类型）；
        // - 布局必须与 C runtime 一致，否则 `scoop_alloc` 初始化的对象头会被错误解释。
        const TY_NAME: &str = "scoop.runtime.ScoopGcObjectHeader";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { void* next; void* type_desc; uint64_t size_bytes; uint32_t flags; uint32_t mark; } ScoopGcObjectHeader;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        ty.set_body(
            &[
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_scoop_type_descriptor_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（ABI 已在 T1501 固化）；
        // - 这里只需要保证字段顺序与大小匹配；具体偏移由 runtime 的 `_Static_assert` 与
        //   `crates/scoop_runtime/tests/object_model_abi.rs` 双向约束。
        const TY_NAME: &str = "scoop.runtime.ScoopTypeDescriptor";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let u64_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        // self-referential：parent_type_desc 指向同一 struct 类型。
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        // 字段顺序必须与 C 定义一致（见 `runtime/c/scoop_gc.h`）。
        ty.set_body(
            &[
                i32_ty.into(),      // abi_version
                i32_ty.into(),      // flags
                i64_ty.into(),      // size_bytes
                i64_ty.into(),      // align_bytes
                i64_ty.into(),      // trace_start_offset_bytes
                i32_ty.into(),      // trace_bitmap_u64_len
                i32_ty.into(),      // _reserved_u32
                u64_ptr_ty.into(),  // trace_bitmap (const uint64_t*)
                i8_ptr_ty.into(),   // trace_fn
                i8_ptr_ty.into(),   // release_fn
                i64_ty.into(),      // type_id
                desc_ptr_ty.into(), // parent_type_desc
                i8_ptr_ty.into(),   // itable
                i8_ptr_ty.into(),   // vtable
            ],
            false,
        );

        ty
    }

    pub(super) fn collect_gc_ptr_offsets_in_basic_type(
        &self,
        at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
        base_off: u64,
        out: &mut Vec<u64>,
    ) -> Result<(), LlvmEmitError> {
        match ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push(base_off);
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                let fields = st.get_field_types();
                for (idx, field_ty) in fields.into_iter().enumerate() {
                    let off = self.target_data.offset_of_element(&st, idx as u32).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "type descriptor field offset",
                            at: at.into(),
                        },
                    )?;
                    self.collect_gc_ptr_offsets_in_basic_type(at, field_ty, base_off + off, out)?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                let elem = arr.get_element_type();
                let stride = self.target_data.get_store_size(&elem);
                let len = arr.len() as u64;
                for i in 0..len {
                    let elem_off = base_off + i.saturating_mul(stride);
                    self.collect_gc_ptr_offsets_in_basic_type(at, elem, elem_off, out)?;
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(())
    }

    pub(super) fn trace_bitmap_words_for_struct(
        &self,
        at: crate::span::Span,
        obj_ty: StructType<'ctx>,
        trace_start_offset_bytes: u64,
    ) -> Result<Vec<u64>, LlvmEmitError> {
        if obj_ty.is_opaque() {
            return Ok(Vec::new());
        }

        let ptr_size = self.target_layout().pointer_size.max(1);
        let size_bytes = self.target_data.get_store_size(&obj_ty);
        if trace_start_offset_bytes >= size_bytes {
            return Ok(Vec::new());
        }
        if !trace_start_offset_bytes.is_multiple_of(ptr_size) {
            return Ok(Vec::new());
        }

        let mut ptr_offsets: Vec<u64> = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(at, obj_ty.into(), 0, &mut ptr_offsets)?;
        ptr_offsets.sort();
        ptr_offsets.dedup();

        let mut word_indices: Vec<u64> = Vec::new();
        for off in ptr_offsets {
            if off < trace_start_offset_bytes {
                continue;
            }
            let rel = off - trace_start_offset_bytes;
            if !rel.is_multiple_of(ptr_size) {
                continue;
            }
            word_indices.push(rel / ptr_size);
        }

        word_indices.sort();
        word_indices.dedup();
        let Some(&max_idx) = word_indices.last() else {
            return Ok(Vec::new());
        };

        let len_u64 = (max_idx / 64) + 1;
        let mut words = vec![0u64; len_u64 as usize];
        for idx in word_indices {
            let wi = (idx / 64) as usize;
            let bit = (idx % 64) as u32;
            words[wi] |= 1u64 << bit;
        }
        Ok(words)
    }

    pub(super) fn get_or_create_trace_bitmap_global(
        &mut self,
        name: &str,
        words: &[u64],
    ) -> GlobalValue<'ctx> {
        if let Some(existing) = self.module.get_global(name) {
            return existing;
        }

        let i64_ty = self.context.i64_type();
        let arr_ty = i64_ty.array_type(words.len() as u32);
        let gv = self.module.add_global(arr_ty, None, name);

        let mut inits: Vec<IntValue<'ctx>> = Vec::with_capacity(words.len());
        for &w in words {
            inits.push(i64_ty.const_int(w, false));
        }

        gv.set_initializer(&i64_ty.const_array(&inits));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        gv
    }

    pub(super) fn get_or_create_type_descriptor_global(
        &mut self,
        spec: TypeDescriptorSpec<'ctx, '_>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let TypeDescriptorSpec {
            at,
            global_name,
            canonical_name,
            obj_ty,
            trace_start_offset_bytes,
            parent,
            itable,
            vtable,
        } = spec;
        if let Some(existing) = self.module.get_global(global_name) {
            return Ok(existing);
        }

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let u64_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        let size_bytes = self.target_data.get_store_size(&obj_ty);
        let align_bytes = self.target_layout().pointer_align.max(1);

        let bitmap_words =
            self.trace_bitmap_words_for_struct(at, obj_ty, trace_start_offset_bytes)?;
        let (bitmap_len_u32, bitmap_ptr) = if bitmap_words.is_empty() {
            (0u32, u64_ptr_ty.const_null())
        } else {
            let bitmap_name = format!("{global_name}__trace_bitmap");
            let bitmap_gv = self.get_or_create_trace_bitmap_global(&bitmap_name, &bitmap_words);
            let ptr = bitmap_gv.as_pointer_value().const_cast(u64_ptr_ty);
            (bitmap_words.len() as u32, ptr)
        };

        let parent_ptr = parent
            .map(|p| p.as_pointer_value())
            .unwrap_or_else(|| desc_ptr_ty.const_null());

        let itable_ptr = itable.unwrap_or_else(|| i8_ptr_ty.const_null());
        let vtable_ptr = vtable.unwrap_or_else(|| i8_ptr_ty.const_null());

        let values: [BasicValueEnum<'ctx>; 14] = [
            i32_ty.const_zero().into(), // abi_version
            i32_ty.const_zero().into(), // flags
            i64_ty.const_int(size_bytes, false).into(),
            i64_ty.const_int(align_bytes, false).into(),
            i64_ty.const_int(trace_start_offset_bytes, false).into(),
            i32_ty.const_int(bitmap_len_u32 as u64, false).into(),
            i32_ty.const_zero().into(), // _reserved_u32
            bitmap_ptr.into(),
            i8_ptr_ty.const_null().into(), // trace_fn
            i8_ptr_ty.const_null().into(), // release_fn
            i64_ty
                .const_int(stable_hash64(canonical_name), false)
                .into(),
            parent_ptr.into(),
            itable_ptr.into(),
            vtable_ptr.into(),
        ];

        let init = desc_ty.const_named_struct(&values);
        let gv = self.module.add_global(desc_ty, None, global_name);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(gv)
    }

    pub(super) fn basic_type_contains_gc_ptrs(
        &self,
        at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
    ) -> Result<bool, LlvmEmitError> {
        let mut ptr_offsets = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(at, ty, 0, &mut ptr_offsets)?;
        Ok(!ptr_offsets.is_empty())
    }

    pub(super) fn get_or_create_global_root_type_desc_global(
        &mut self,
        at: crate::span::Span,
        global_name: &str,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if !self.basic_type_contains_gc_ptrs(at, storage_ty)? {
            return Ok(None);
        }

        let wrapper_ty = self.context.struct_type(&[storage_ty], false);
        let desc_name = format!("{global_name}__global_root_type_desc");
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &desc_name,
            canonical_name: global_name,
            obj_ty: wrapper_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: None,
            vtable: None,
        })
        .map(Some)
    }

    pub(super) fn get_or_create_class_type_desc_global(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!(
            "__scoop_type_desc_class__{}",
            sanitize_llvm_ident(class_fqn)
        );
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let class = self.class_init_layout(at, class_fqn)?;
        let parent = if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            Some(self.get_or_create_class_type_desc_global(at, super_fqn)?)
        } else {
            None
        };

        let obj_ty = self.llvm_class_object_type(at, &class)?;
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);

        let itable_ptr = self
            .get_or_create_class_itable_global(at, class_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        let vtable_ptr = self
            .get_or_create_class_vtable_global(at, class_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &class.fqn,
            obj_ty,
            trace_start_offset_bytes,
            parent,
            itable: itable_ptr,
            vtable: vtable_ptr,
        })
    }

    pub(super) fn llvm_object_singleton_type(&self, object_fqn: &str) -> StructType<'ctx> {
        let name = format!(
            "scoop.runtime.ObjectSingleton__{}",
            sanitize_llvm_ident(object_fqn)
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into()], false);
        ty
    }

    pub(super) fn get_or_create_object_singleton_type_desc_global(
        &mut self,
        at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!(
            "__scoop_type_desc_object__{}",
            sanitize_llvm_ident(object_fqn)
        );
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_object_singleton_type(object_fqn);
        let itable_ptr = self
            .get_or_create_class_itable_global(at, object_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));
        let vtable_ptr = self
            .get_or_create_class_vtable_global(at, object_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: object_fqn,
            obj_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: itable_ptr,
            vtable: vtable_ptr,
        })
    }

    pub(super) fn get_or_create_class_itable_global(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let Some(entries) = self.class_itables.get(class_fqn) else {
            return Ok(None);
        };
        if entries.is_empty() {
            return Ok(None);
        }

        let global_name = format!("__scoop_itable__{}", sanitize_llvm_ident(class_fqn));
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(Some(existing));
        }

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // itable entry：
        // { interface_id: u64, match_len: u32, _reserved: u32, match_ids: i8*, methods: i8* }
        //
        // 说明：
        // - `methods` 指向一个 `i8*[]`（函数指针数组），按 interface slot 顺序排列；
        // - `match_ids` 指向一个 `u64[]`，保存该具体 interface 实例在运行期可匹配的 target type ids。
        let entry_ty = self.context.struct_type(
            &[
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
            ],
            false,
        );

        let mut entry_inits: Vec<inkwell::values::StructValue<'ctx>> =
            Vec::with_capacity(entries.len());

        for entry in entries {
            // 1) 生成 method table：`i8*[]`。
            let methods_gv_name = format!(
                "__scoop_itable_methods__{}__{:016x}__{:016x}",
                sanitize_llvm_ident(class_fqn),
                entry.interface_id,
                entry.interface_type_id,
            );

            let methods_gv = if let Some(existing) = self.module.get_global(&methods_gv_name) {
                existing
            } else {
                let arr_ty = i8_ptr_ty.array_type(entry.method_impl_fqns.len() as u32);
                let gv = self.module.add_global(arr_ty, None, &methods_gv_name);

                let mut inits: Vec<PointerValue<'ctx>> =
                    Vec::with_capacity(entry.method_impl_fqns.len());
                for impl_fqn in &entry.method_impl_fqns {
                    if impl_fqn.is_empty() {
                        inits.push(i8_ptr_ty.const_null());
                        continue;
                    }

                    let sig_fun = self.fun_index.get(impl_fqn.as_str()).copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "class itable slot target",
                            at: at.into(),
                        },
                    )?;

                    let llvm_name = self
                        .extern_funs
                        .get(impl_fqn)
                        .map(|e| e.symbol.as_str())
                        .unwrap_or(impl_fqn.as_str());

                    let llvm_fun = match self.module.get_function(llvm_name) {
                        Some(f) => f,
                        None => self.declare_top_level_fun(sig_fun)?,
                    };

                    let fn_ptr = llvm_fun.as_global_value().as_pointer_value();
                    inits.push(fn_ptr.const_cast(i8_ptr_ty));
                }

                gv.set_initializer(&i8_ptr_ty.const_array(&inits));
                gv.set_constant(true);
                gv.set_linkage(Linkage::Internal);
                gv
            };

            let methods_ptr_i8 = methods_gv.as_pointer_value().const_cast(i8_ptr_ty).into();

            let match_ids_ptr_i8 = if entry.runtime_match_type_ids.is_empty() {
                i8_ptr_ty.const_null().into()
            } else {
                let match_gv_name = format!(
                    "__scoop_itable_match_ids__{}__{:016x}__{:016x}",
                    sanitize_llvm_ident(class_fqn),
                    entry.interface_id,
                    entry.interface_type_id,
                );
                let match_gv = if let Some(existing) = self.module.get_global(&match_gv_name) {
                    existing
                } else {
                    let arr_ty = i64_ty.array_type(entry.runtime_match_type_ids.len() as u32);
                    let gv = self.module.add_global(arr_ty, None, &match_gv_name);
                    let inits = entry
                        .runtime_match_type_ids
                        .iter()
                        .map(|id| i64_ty.const_int(*id, false))
                        .collect::<Vec<_>>();
                    gv.set_initializer(&i64_ty.const_array(&inits));
                    gv.set_constant(true);
                    gv.set_linkage(Linkage::Internal);
                    gv
                };
                match_gv.as_pointer_value().const_cast(i8_ptr_ty).into()
            };

            let init = entry_ty.const_named_struct(&[
                i64_ty.const_int(entry.interface_id, false).into(),
                i32_ty
                    .const_int(entry.runtime_match_type_ids.len() as u64, false)
                    .into(),
                i32_ty.const_zero().into(),
                match_ids_ptr_i8,
                methods_ptr_i8,
            ]);
            entry_inits.push(init);
        }

        let entries_arr_ty = entry_ty.array_type(entry_inits.len() as u32);
        let entries_arr_init = entry_ty.const_array(&entry_inits);

        // itable：{ len: i32, _reserved: i32, entries: [N x Entry] }
        let itable_ty = self.context.struct_type(
            &[i32_ty.into(), i32_ty.into(), entries_arr_ty.into()],
            false,
        );
        let itable_init = itable_ty.const_named_struct(&[
            i32_ty.const_int(entries.len() as u64, false).into(),
            i32_ty.const_zero().into(),
            entries_arr_init.into(),
        ]);

        let gv = self.module.add_global(itable_ty, None, &global_name);
        gv.set_initializer(&itable_init);
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(Some(gv))
    }

    pub(super) fn get_or_create_class_vtable_global(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let Some(slots) = self.class_vtables.get(class_fqn) else {
            return Ok(None);
        };
        if slots.is_empty() {
            return Ok(None);
        }

        let global_name = format!("__scoop_vtable__{}", sanitize_llvm_ident(class_fqn));
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(Some(existing));
        }

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let arr_ty = i8_ptr_ty.array_type(slots.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &global_name);

        let mut inits: Vec<PointerValue<'ctx>> = Vec::with_capacity(slots.len());
        for slot in slots {
            let sig_fun = self
                .fun_index
                .get(slot.impl_member_fqn.as_str())
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class vtable slot target",
                    at: at.into(),
                })?;

            let llvm_name = self
                .extern_funs
                .get(&slot.impl_member_fqn)
                .map(|e| e.symbol.as_str())
                .unwrap_or(slot.impl_member_fqn.as_str());

            let llvm_fun = match self.module.get_function(llvm_name) {
                Some(f) => f,
                None => self.declare_top_level_fun(sig_fun)?,
            };

            let fn_ptr = llvm_fun.as_global_value().as_pointer_value();
            inits.push(fn_ptr.const_cast(i8_ptr_ty));
        }

        gv.set_initializer(&i8_ptr_ty.const_array(&inits));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(Some(gv))
    }

    pub(super) fn get_or_create_closure_object_type_desc_global(
        &mut self,
        at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__ScoopClosure";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_closure_object_type();
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: GLOBAL_NAME,
            canonical_name: "scoop.runtime.ScoopClosure",
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_closure_env_type_desc_global(
        &mut self,
        at: crate::span::Span,
        closure_id: hir::ClosureId,
        env_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!("__scoop_type_desc_closure_env__{}", closure_id.as_u32());
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let trace_start_offset_bytes = self.target_data.offset_of_element(&env_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &format!("scoop.lambda_env${}", closure_id.as_u32()),
            obj_ty: env_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_string_type_desc_global(
        &mut self,
        at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__ScoopString";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_scoop_string_type();
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: GLOBAL_NAME,
            canonical_name: "scoop.core.String",
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn llvm_boxed_unit_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.BoxedUnit";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into()], false);
        ty
    }

    pub(super) fn get_or_create_boxed_unit_type_desc_global(
        &mut self,
        at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__BoxedUnit";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_boxed_unit_type();
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: GLOBAL_NAME,
            canonical_name: "scoop.runtime.BoxedUnit",
            obj_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_boxed_int_type_desc_global(
        &mut self,
        at: crate::span::Span,
        payload: IntTy,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!(
            "__scoop_type_desc_runtime__boxed_int{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_boxed_int_type(payload);
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &format!(
                "scoop.runtime.BoxedInt{}_{}",
                payload.bits,
                if payload.signed { "i" } else { "u" }
            ),
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn llvm_boxed_int_type(&self, payload: IntTy) -> StructType<'ctx> {
        // 说明：box 类型目前只用于 `Int/UInt/... -> Any` 的最小装箱（T0817）。
        // 未来会扩展为统一的对象头 + type descriptor（T0907+）；当前已接入最小对象头（T0908）。
        let name = format!(
            "scoop.runtime.BoxedInt{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        // `{ ScoopGcObjectHeader header, <int> payload }`
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), self.int_type(payload).into()], false);
        ty
    }

    pub(super) fn llvm_closure_object_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型是 early stage 的函数值/闭包运行期表示（T0710/T1307b）。
        // - env 指针指向一个 GC-managed 的 closure env heap object（无捕获时为 NULL）。
        //
        // 布局（与 GC 对象头兼容）：
        // `{ header: ScoopGcObjectHeader, env_ptr: i8 addrspace(1)*, fn_ptr: i8* }`
        const TY_NAME: &str = "scoop.runtime.ScoopClosure";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        ty.set_body(
            &[header_ty.into(), gc_i8_ptr_ty.into(), i8_ptr_ty.into()],
            false,
        );
        ty
    }

    pub(super) fn store_local_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 说明：当前阶段 locals 允许：
        // - 标量：`Unit/Bool/Int*`
        // - struct/enum（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        self.store_local_value_exact(at, ptr, ty, v)
    }

    pub(super) fn store_local_value_exact(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // safepoint 是 GC ref 的 clobber 边界：若 `ptr` 由某个可被 moving GC 更新的
        // base 指针（例如 explicit-frame-backed 的 heap frame ptr load）推导而来，
        // 则不能继续信任旧 SSA/GEP；在真正写入前需要在当前 block 里重建指针链。
        let ptr = self.rematerialize_ptr_in_current_block(at, ptr, "store_local_slot")?;

        if value.ty != ty && value.ty != CgTy::Never {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "store exact value type mismatch",
                at: at.into(),
            });
        }

        match ty {
            // T1612: Nothing/Never has no runtime value; storing is a no-op (unreachable path).
            CgTy::Never => return Ok(CgValue::never()),
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "store value",
                        at: at.into(),
                    });
                };
                // T1412d：当写入目标位于 GC heap（addrspace(1)）且写入值为 GC ref 时，
                // 必须走统一写屏障 hook，避免形成 old→nursery 指针（minor GC 的关键前置条件）。
                //
                // 注意：locals/alloca 在 addrspace(0)，因此不会触发该分支。
                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && needs_write_barrier_for_value_ty(self, at, ty)?
                {
                    let BasicValueEnum::PointerValue(value_ptr) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "write barrier value type (ptr)",
                            at: at.into(),
                        });
                    };

                    self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)?;
                } else {
                    let store_inst = self.builder.build_store(ptr, raw)?;
                    // T0119: `@CLayout(packed = N)` — aggregate store 到 alloca 时，
                    // store alignment 降到 packed value（与 load 路径保持一致）。
                    // packed=1 时 alignment=1，packed>1 时 alignment=min(struct_natural, N)。
                    if let CgTy::Struct(struct_ty) = ty
                        && let Some(pack_n) = self.struct_clayout(struct_ty).and_then(|c| c.packed)
                    {
                        // For whole-aggregate store, use pack_n as alignment
                        // (the struct is packed, so its overall alignment is at most pack_n).
                        store_inst.set_alignment(pack_n)?;
                    }
                }
                if ptr.get_type().get_address_space() == AddressSpace::default() {
                    let storage_ty = self.llvm_basic_type_of(at, ty)?;
                    self.sync_storage_slot_into_explicit_frame(at, ptr, storage_ty, "store_local")?;
                }
            }
            CgTy::Enum(enum_ty) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "store value",
                        at: at.into(),
                    });
                };

                if self.try_store_heap_tagged_union_enum_exact(at, ptr, enum_ty, raw)? {
                    return Ok(value);
                }

                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && needs_write_barrier_for_value_ty(self, at, ty)?
                {
                    let BasicValueEnum::PointerValue(value_ptr) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "write barrier value type (ptr)",
                            at: at.into(),
                        });
                    };

                    self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)?;
                } else {
                    let _ = self.builder.build_store(ptr, raw)?;
                }
                if ptr.get_type().get_address_space() == AddressSpace::default() {
                    let storage_ty = self.llvm_basic_type_of(at, ty)?;
                    self.sync_storage_slot_into_explicit_frame(at, ptr, storage_ty, "store_enum")?;
                }
            }
        }
        Ok(value)
    }

    pub(super) fn store_gc_pointer_slot_with_write_barrier(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let wb = self.declare_runtime_gc_write_barrier();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // `slot_addr`：传入“slot 的地址”即可；runtime 用 memcpy 写回（避免 strict alias UB）。
        //
        // 注意：该地址只是 native 指针（C ABI `void*`），不应位于 GC address space；
        // 否则会被 statepoint/stackmap 当作 GC root，产生 derived/non-header roots。
        let slot_addr_i8_gc =
            self.builder
                .build_pointer_cast(ptr, gc_i8_ptr_ty, "gc_wb_slot_addr_i8_gc")?;
        let slot_addr =
            self.builder
                .build_address_space_cast(slot_addr_i8_gc, i8_ptr_ty, "gc_wb_slot_addr")?;
        let value_i8 =
            self.builder
                .build_pointer_cast(value_ptr, gc_i8_ptr_ty, "gc_wb_value_i8")?;

        let _ = self.build_call_preserving_gc_local_roots(
            at,
            wb,
            &[slot_addr.into(), value_i8.into()],
            "gc_write_barrier",
        )?;
        Ok(())
    }

    fn try_store_heap_tagged_union_enum_exact(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        enum_ty: crate::ty::TypeId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<bool, LlvmEmitError> {
        if ptr.get_type().get_address_space() != self.gc_address_space() {
            return Ok(false);
        }

        let layout = self.cg_enum_layout(at, enum_ty)?;
        if !matches!(layout.repr, CgEnumRepr::TaggedUnion) {
            return Ok(false);
        }

        let BasicValueEnum::StructValue(enum_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tagged union enum heap store",
                at: at.into(),
            });
        };

        let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
        let gc_ptr_slot = self
            .builder
            .build_struct_gep(llvm_enum_ty, ptr, 2, "enum_gc_ptr_gep")?;
        let word_slot =
            self.builder
                .build_struct_gep(llvm_enum_ty, ptr, 1, "enum_payload_word_gep")?;
        let tag_slot = self
            .builder
            .build_struct_gep(llvm_enum_ty, ptr, 0, "enum_tag_gep")?;

        // 先写 GC pointer 槽位：若写屏障内部触发 GC，GC 至少还能通过静态 layout 看到新 payload。
        let gc_ptr = self
            .builder
            .build_extract_value(enum_raw, 2, "enum_payload_ptr")?;
        let BasicValueEnum::PointerValue(gc_ptr) = gc_ptr else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tagged union enum gc payload field",
                at: at.into(),
            });
        };
        self.store_gc_pointer_slot_with_write_barrier(at, gc_ptr_slot, gc_ptr)?;

        let word = self
            .builder
            .build_extract_value(enum_raw, 1, "enum_payload_word")?;
        let BasicValueEnum::IntValue(word) = word else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tagged union enum word payload field",
                at: at.into(),
            });
        };
        let _ = self.builder.build_store(word_slot, word)?;

        let tag = self.builder.build_extract_value(enum_raw, 0, "enum_tag")?;
        let BasicValueEnum::IntValue(tag) = tag else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tagged union enum tag field",
                at: at.into(),
            });
        };
        let _ = self.builder.build_store(tag_slot, tag)?;

        Ok(true)
    }
}

fn needs_write_barrier_for_value_ty<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    at: crate::span::Span,
    ty: CgTy,
) -> Result<bool, LlvmEmitError> {
    // 说明：
    // - `write_barrier(slot, value)` 语义上针对“写入 slot 的 GC-managed 指针”；
    // - 大多数情况下，这对应 `CgTy::Ref/String`；
    // - 但 `Option<Ref>` 这类 enum 可能通过 niche 优化降为“直接用 payload 指针承载 enum 值”，
    //   在 LLVM IR 侧同样表现为 `ptr addrspace(1)`；
    //   若仅按 `CgTy::Ref/String` 判断，会漏掉这类 heap field store，从而在 `--gc-stress` 下出现回归。
    // - 更复杂的 tagged union enum 会在 `store_local_value_exact` 中拆成
    //   `tag/word/gc_ptr` 三槽写回；这里保留“单指针 store”子集。
    match ty {
        CgTy::Ref | CgTy::String => Ok(true),
        CgTy::Enum(enum_ty) => {
            // 仅处理“niche pointer enum，且 payload 是 GC 指针”的子集。
            let layout = cg.cg_enum_layout(at, enum_ty)?;
            match layout.repr {
                CgEnumRepr::Niche {
                    storage: NicheStorage::Pointer,
                    ..
                } => {
                    let some_field_is_gc_ptr = layout
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter())
                        .any(|f| matches!(f, CgTy::Ref | CgTy::String));
                    Ok(some_field_is_gc_ptr)
                }
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}
