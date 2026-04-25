//! Channel intrinsics lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_channels_channel_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_channels_channel_create();
        let call = self
            .builder
            .build_call(rt, &[], "channels_channel_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_channels_send(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(value_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send named arg (value)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send receiver type",
                at: channel_expr.span.into(),
            });
        };

        // 优先从 receiver 的静态类型恢复 `T`，以便对 `value` 施加期望类型与编码方式。
        //
        // 注意（GC-FIX C2b）：当前 runtime 的 channel nodes 用 `malloc/free` 管理且不参与 GC trace，
        // 因此这里暂只允许 word payload；Ref/String 若进入队列会变成 silent roots hole。
        let elem_cg = match self.types.kind(channel_expr.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.channels.Channel" && nominal.args.len() == 1 =>
            {
                self.cg_ty_of(nominal.args[0])
                    .filter(|ty| matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_)))
            }
            _ => None,
        };

        let value_v = match elem_cg {
            Some(elem_cg) => {
                let v = self.codegen_expr_in_expected_context(value_expr, Some(elem_cg))?;
                self.coerce_value(value_expr.span, v, elem_cg)?
            }
            None => self.codegen_expr(value_expr)?,
        };
        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;

        let rt = self.declare_runtime_channels_send_u64();
        let call = self.builder.build_call(
            rt,
            &[channel_ptr.into(), word_u64.into()],
            "channels_send_u64",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "channels_send_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_channels_recv(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv named arg (receiver)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv receiver type",
                at: channel_expr.span.into(),
            });
        };

        // 恢复 `T`：优先从 receiver 的静态类型 `Channel<T>` 得到；若无法恢复，则退化使用 expected context
        //（例如 `val v: Int? = ch.recv()`）从 `Option<T>` 里反推 `T`。
        let (option_ty, elem_ty) = match self.types.kind(channel_expr.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.channels.Channel" && nominal.args.len() == 1 =>
            {
                let elem_ty = nominal.args[0];
                let option_ty = self
                    .types
                    .iter_ids()
                    .find(|id| match self.types.kind(*id) {
                        TypeKind::Value(ValueTypeKind::Option(inner)) => *inner == elem_ty,
                        _ => false,
                    })
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "channels.Channel.recv return Option<T> type",
                        at: span.into(),
                    })?;
                (option_ty, elem_ty)
            }
            _ => match expected {
                Some(CgTy::Enum(option_ty)) => match self.types.kind(option_ty) {
                    TypeKind::Value(ValueTypeKind::Option(inner)) => (option_ty, *inner),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "channels.Channel.recv expected Option<T>",
                            at: span.into(),
                        });
                    }
                },
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "channels.Channel.recv receiver nominal",
                        at: channel_expr.span.into(),
                    });
                }
            },
        };

        // gate：确保元素是 "u64 word 可编码"的类型（与 `coerce_u64_word` 对齐）。
        let elem_cg = self
            .cg_ty_of(elem_ty)
            .filter(|ty| matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_)));
        if elem_cg.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv element type",
                at: channel_expr.span.into(),
            });
        }

        // `uint32_t scoop_channels_recv_u64(void* channel, uint64_t* out_value)`
        let i64_ty = self.context.i64_type();
        let out_ptr = self.create_entry_alloca_raw(span, "channels_recv_out", i64_ty.into())?;

        let rt = self.declare_runtime_channels_recv_u64();
        let call = self.builder.build_call(
            rt,
            &[channel_ptr.into(), out_ptr.into()],
            "channels_recv_u64",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "channels_recv_ok",
        )?;

        let option_cg = CgTy::Enum(option_ty);
        let option_llvm_ty = self.llvm_basic_type_of(span, option_cg)?;

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

        let some_bb = self.context.append_basic_block(func, "channels_recv_some");
        let none_bb = self.context.append_basic_block(func, "channels_recv_none");
        let merge_bb = self.context.append_basic_block(func, "channels_recv_merge");

        self.builder
            .build_conditional_branch(ok_cond, some_bb, none_bb)?;

        // some branch：读取 word，构造 `Some(value)`。
        self.builder.position_at_end(some_bb);
        let word_u64 = self
            .builder
            .build_load(i64_ty, out_ptr, "channels_recv_word")?
            .into_int_value();
        let from = IntTy {
            bits: 64,
            signed: false,
        };
        let payload_ty = self.enum_payload_ty();
        let payload_word = self.cast_int(word_u64, from, payload_ty)?;
        let some_v = self.build_enum_value(
            span,
            option_ty,
            0,
            CgEnumPayload {
                word: Some(payload_word),
                gc_ptr: None,
            },
        )?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let some_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        // none branch：构造 `None`。
        self.builder.position_at_end(none_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let none_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        // merge：phi 合并结果。
        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(option_llvm_ty, "channels_recv_phi")?;
        phi.add_incoming(&[(&some_raw, some_end), (&none_raw, none_end)]);

        Ok(CgValue {
            ty: option_cg,
            value: Some(phi.as_basic_value()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_channels_close(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close named arg (receiver)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close receiver type",
                at: channel_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_channels_close();
        let _ = self
            .builder
            .build_call(rt, &[channel_ptr.into()], "channels_close")?;
        Ok(CgValue::unit())
    }
}
