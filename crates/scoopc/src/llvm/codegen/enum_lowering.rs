//! Enum constructor, payload coercion, and enum-constant lowering split out of `codegen/mod.rs`.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_unresolved_ident(
        &mut self,
        span: crate::span::Span,
        name: &str,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 0-参数 enum variant 值：`None`
        let Some(CgTy::Enum(enum_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident without expected enum type",
                at: span.into(),
            });
        };

        let cg_layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = cg_layout.variants.iter().find(|v| v.name == name).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            },
        )?;
        let tag = variant.tag;
        let field_count = variant.fields.len();

        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "non-zero-arity enum variant used as value",
                at: span.into(),
            });
        }

        self.build_enum_value(span, enum_ty, tag, CgEnumPayload::default())
    }

    pub(in crate::llvm::codegen) fn codegen_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|v| v.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            })?
            .clone();

        if variant.fields.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant ctor arity mismatch",
                at: span.into(),
            });
        }

        // 先把所有实参在"字段期望类型"下 codegen 并做最小 coercion，避免后续重复走 codegen。
        let mut field_values: Vec<(crate::span::Span, CgTy, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(args.len());
        for (idx, (field_cg, arg)) in variant.fields.iter().copied().zip(args.iter()).enumerate() {
            let hir::CallArg::Positional(arg_expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named enum ctor arg",
                    at: span.into(),
                });
            };

            let v = self.codegen_expr_in_expected_context(arg_expr, Some(field_cg))?;
            let coerced = self.coerce_value(arg_expr.span, v, field_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg_expr.span,
                &format!("enum_ctor_field_{idx}"),
                coerced,
            )?;
            field_values.push((arg_expr.span, field_cg, deferred));

            // 提前在 debug 名称里体现 index，便于排查（不影响语义）。
            let _ = idx;
        }

        self.build_enum_variant_value_from_field_values(span, enum_ty, variant_name, &field_values)
    }

    pub(in crate::llvm::codegen) fn build_enum_variant_value_from_field_values(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        field_values: &[(crate::span::Span, CgTy, DeferredCgValue<'ctx>)],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|v| v.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            })?
            .clone();

        if variant.fields.len() != field_values.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant ctor arity mismatch",
                at: span.into(),
            });
        }

        // 1) boxed variant：把 payload fields 聚合成一个 payload struct，存到栈上并把指针写入 enum payload。
        if variant.boxed {
            // GC safety（T1516）：
            // - boxed payload 不能暂存在栈上然后把栈指针塞进 enum 的 word payload；
            //   否则其中的 GC refs 无法被 stackmap/bitmap 扫描，触发 GC 后会出现悬挂指针。
            // - 因此 boxed payload 必须是一个 GC-managed heap object，并把对象指针写入 enum 的
            //   GC pointer slot（payload_ptr）。
            let payload_struct_ty =
                self.llvm_enum_boxed_payload_struct_type(span, enum_ty, &variant)?;
            let payload_obj_ty =
                self.llvm_enum_boxed_payload_object_type(span, enum_ty, &variant)?;
            let obj_size_bytes = self.target_data.get_store_size(&payload_obj_ty);
            let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

            let desc = self.get_or_create_enum_boxed_payload_type_desc_global(
                span,
                enum_ty,
                &variant,
                payload_obj_ty,
            )?;
            let desc_i8 = self.builder.build_pointer_cast(
                desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "enum_boxed_payload_type_desc_i8",
            )?;
            let rt_alloc = self.declare_runtime_alloc_typed();
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[desc_i8.into(), size_v.into()],
                "rt_alloc_enum_boxed_payload",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value (enum boxed payload)",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(raw_ptr) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type (enum boxed payload)",
                    at: span.into(),
                });
            };

            let mut payload: AggregateValueEnum<'ctx> = payload_struct_ty.get_undef().into();
            for (idx, (field_span, field_cg, deferred)) in field_values.iter().enumerate() {
                let field_v = self.materialize_deferred_cg_value(
                    *field_span,
                    &format!("enum_ctor_field_reload_{idx}"),
                    deferred.clone(),
                )?;
                // Unit 没有运行期值；当前阶段不允许把 Unit 作为 enum payload 字段。
                if matches!(field_cg, CgTy::Unit) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum boxed payload field (unit)",
                        at: span.into(),
                    });
                }
                let raw = field_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum boxed payload field value",
                    at: span.into(),
                })?;
                payload = self.builder.build_insert_value(
                    payload,
                    raw,
                    idx as u32,
                    &format!("enum_payload_field_{idx}"),
                )?;
            }

            let payload_obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let payload_obj_ptr = self.builder.build_pointer_cast(
                raw_ptr,
                payload_obj_ptr_ty,
                "enum_boxed_payload_obj_ptr",
            )?;
            let payload_gep = self.builder.build_struct_gep(
                payload_obj_ty,
                payload_obj_ptr,
                1,
                "enum_boxed_payload_gep",
            )?;
            let _ = self
                .builder
                .build_store(payload_gep, payload.as_basic_value_enum())?;

            let payload_ptr_ty = self.llvm_gc_i8_ptr_type();
            let payload_ptr_i8 = self.builder.build_pointer_cast(
                payload_obj_ptr,
                payload_ptr_ty,
                "enum_boxed_payload_as_i8",
            )?;

            let word_ty = self.int_type(self.enum_payload_ty());
            let payload_word = word_ty.const_int(0, false);
            return self.build_enum_value(
                span,
                enum_ty,
                variant.tag,
                CgEnumPayload {
                    word: Some(payload_word),
                    gc_ptr: Some(payload_ptr_i8),
                },
            );
        }

        let field_values = field_values
            .iter()
            .enumerate()
            .map(|(idx, (field_span, field_cg, deferred))| {
                let materialized = self.materialize_deferred_cg_value(
                    *field_span,
                    &format!("enum_ctor_field_reload_{idx}"),
                    deferred.clone(),
                )?;
                Ok((*field_cg, materialized))
            })
            .collect::<Result<Vec<_>, LlvmEmitError>>()?;

        // 2) inline（非 boxed）variant：当前阶段仍采用 "word payload" 承载的小 payload。
        if variant.fields.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant payload (multi-field, not boxed)",
                at: span.into(),
            });
        }

        let payload = if let Some((field_cg, field_v)) = field_values.first().copied() {
            self.coerce_enum_payload(span, field_v, field_cg)?
        } else {
            CgEnumPayload::default()
        };

        self.build_enum_value(span, enum_ty, variant.tag, payload)
    }

    pub(in crate::llvm::codegen) fn coerce_enum_payload(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        value_ty: CgTy,
    ) -> Result<CgEnumPayload<'ctx>, LlvmEmitError> {
        let payload_ty = self.enum_payload_ty();
        let payload_int_ty = self.int_type(payload_ty);

        match value_ty {
            CgTy::Unit | CgTy::Never => Ok(CgEnumPayload {
                word: Some(payload_int_ty.const_int(0, false)),
                gc_ptr: None,
            }),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload bool",
                    at: at.into(),
                })?;
                let widened =
                    self.builder
                        .build_int_z_extend(b, payload_int_ty, "enum_payload_bool")?;
                Ok(CgEnumPayload {
                    word: Some(widened),
                    gc_ptr: None,
                })
            }
            CgTy::Int(from) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload int",
                    at: at.into(),
                })?;
                if from.bits > payload_ty.bits {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload larger than word",
                        at: at.into(),
                    });
                }
                let casted = self.cast_int(v, from, payload_ty)?;
                Ok(CgEnumPayload {
                    word: Some(casted),
                    gc_ptr: None,
                })
            }
            CgTy::Float64 | CgTy::Float32 => {
                let word = self.coerce_u64_word(at, value)?;
                Ok(CgEnumPayload {
                    word: Some(word),
                    gc_ptr: None,
                })
            }
            CgTy::String => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "enum_payload_str_as_ref",
                )?;
                Ok(CgEnumPayload {
                    word: None,
                    gc_ptr: Some(casted),
                })
            }
            CgTy::Ref => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "enum_payload_ref_as_i8",
                )?;
                Ok(CgEnumPayload {
                    word: None,
                    gc_ptr: Some(casted),
                })
            }
            CgTy::Enum(nested_enum_ty) => {
                // 允许把 "niche enum（当前主要是 `Option<...>`）" 作为 payload 承载到外层 enum/Option 中。
                //
                // 关键点：
                // - niche enum 的运行期值本身就是一个"标量存储"（ptr 或 u8）；
                // - 因此可以映射到 tagged union 的 `{ payload_word, payload_ptr }` 载体上，
                //   且不引入 ptr<->int 编码（GC safety）。
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload nested enum",
                        at: at.into(),
                    });
                };

                let repr = self.cg_enum_layout(at, nested_enum_ty)?.repr;
                match repr {
                    CgEnumRepr::Niche {
                        storage,
                        none_value,
                    } => match storage {
                        NicheStorage::Pointer => {
                            // GC safety（T1518）：pointer niche 只允许 `None = NULL`。
                            if none_value != 0 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum pointer none_value (must be NULL)",
                                    at: at.into(),
                                });
                            }

                            let BasicValueEnum::PointerValue(ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum payload (ptr)",
                                    at: at.into(),
                                });
                            };

                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                self.llvm_gc_i8_ptr_type(),
                                "enum_payload_nested_niche_ptr_as_i8",
                            )?;
                            Ok(CgEnumPayload {
                                word: None,
                                gc_ptr: Some(casted),
                            })
                        }
                        NicheStorage::U8 => {
                            let BasicValueEnum::IntValue(v) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum payload (u8)",
                                    at: at.into(),
                                });
                            };
                            let widened = self.builder.build_int_z_extend(
                                v,
                                payload_int_ty,
                                "enum_payload_nested_niche_u8",
                            )?;
                            Ok(CgEnumPayload {
                                word: Some(widened),
                                gc_ptr: None,
                            })
                        }
                    },
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload (nested enum, unsupported repr)",
                        at: at.into(),
                    }),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum payload (non-scalar)",
                at: at.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn build_enum_value(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        tag: u64,
        payload: CgEnumPayload<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 注意：`cg_enum_layout(...)` 当前会返回一份从共享 cache 克隆出来的 layout；
        // 这里仍先提取出后续需要的字段，避免把整份 layout 在长分支里来回搬运。
        let (repr, some_field) = {
            let layout = self.cg_enum_layout(at, enum_ty)?;
            let repr = layout.repr;
            let some_field = layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();

                let tag_ty = self.context.i32_type();
                let payload_word_ty = self.int_type(self.enum_payload_ty());
                let payload_ptr_ty = self.llvm_gc_i8_ptr_type();

                agg = self.builder.build_insert_value(
                    agg,
                    tag_ty.const_int(tag, false),
                    0,
                    "enum_tag",
                )?;

                let payload_word_v = payload
                    .word
                    .unwrap_or_else(|| payload_word_ty.const_int(0, false));
                agg =
                    self.builder
                        .build_insert_value(agg, payload_word_v, 1, "enum_payload_word")?;

                let payload_ptr_v = payload
                    .gc_ptr
                    .unwrap_or_else(|| payload_ptr_ty.const_null());
                agg = self
                    .builder
                    .build_insert_value(agg, payload_ptr_v, 2, "enum_payload_ptr")?;

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                })
            }
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                // 说明：niche 表示下 `tag` 不参与运行期布局；caller 只需要保证：
                // - `None`：payload 传 None（使用 `none_value` 作为编码）；
                // - `Some(x)`：payload 传 Some(word(x))。
                let word_ty = self.int_type(self.enum_payload_ty());
                let raw: BasicValueEnum<'ctx> = match storage {
                    NicheStorage::Pointer => {
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche pointer none_value (must be NULL)",
                                at: at.into(),
                            });
                        }

                        // 存储类型取 `Some` variant 的字段类型（通常为指针）。
                        let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "Option niche payload type",
                            at: at.into(),
                        })?;
                        let llvm_storage_ty = self.llvm_basic_type_of(at, some_field)?;
                        let BasicTypeEnum::PointerType(ptr_ty) = llvm_storage_ty else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche storage (non-pointer)",
                                at: at.into(),
                            });
                        };

                        match tag {
                            0 => {
                                let Some(raw_ptr) = payload.gc_ptr else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "Option niche Some payload missing",
                                        at: at.into(),
                                    });
                                };
                                self.builder
                                    .build_pointer_cast(raw_ptr, ptr_ty, "option_some_cast")?
                                    .into()
                            }
                            1 => ptr_ty.const_null().into(),
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "Option niche tag",
                                    at: at.into(),
                                });
                            }
                        }
                    }
                    NicheStorage::U8 => {
                        let encoded = payload
                            .word
                            .unwrap_or_else(|| word_ty.const_int(none_value, false));
                        self.builder
                            .build_int_truncate(encoded, self.context.i8_type(), "option_niche_u8")?
                            .into()
                    }
                };

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(raw),
                })
            }
            CgEnumRepr::ValueOnly { underlying } => {
                let llvm_ty = self.int_type(underlying);
                let v = llvm_ty.const_int(tag, false);
                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(v.into()),
                })
            }
        }
    }

    /// 将一个"限定名 enum unit variant 值"（例如 `RuntimeError.NullAssertionFailed`）降低为 enum 常量。
    ///
    /// 说明：
    /// - parser 会把 `EnumName.Variant` 表示为 member access；
    /// - resolver 会将 `Variant` 解析为一个 value FQN（`EnumFqn.Variant`）；
    /// - 对于 0-arity（unit）variant，我们在 codegen 侧直接构造 `{ tag, payload }` 值。
    pub(in crate::llvm::codegen) fn try_codegen_qualified_enum_unit_variant_value(
        &mut self,
        at: crate::span::Span,
        value_fqn: &str,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some((owner_fqn, variant_name)) = value_fqn.rsplit_once('.') else {
            return Ok(None);
        };
        let Some(enum_layout) = self.enum_layouts.get(owner_fqn) else {
            return Ok(None);
        };
        let Some(variant) = enum_layout.variants.iter().find(|v| v.name == variant_name) else {
            return Ok(None);
        };

        let tag = variant.tag;
        let field_count = variant.fields.len();
        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant with payload used as value",
                at: at.into(),
            });
        }

        let enum_ty = self
            .types
            .iter_ids()
            .find(|id| {
                matches!(
                    self.types.kind(*id),
                    TypeKind::Value(ValueTypeKind::Nominal(nominal))
                        if nominal.fqn == owner_fqn && nominal.args.is_empty() && nominal.eff.is_none()
                )
            })
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id for qualified variant value",
                at: at.into(),
            })?;

        let v = self.build_enum_value(at, enum_ty, tag, CgEnumPayload::default())?;
        Ok(Some(v))
    }
}
