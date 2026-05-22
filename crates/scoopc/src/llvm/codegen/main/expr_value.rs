//! Expression lowering: var ref, struct/tuple lit, member access, cg_value_from_loaded.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_var_ref(
        &mut self,
        span: crate::span::Span,
        v: &hir::ValueRef,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match v {
            hir::ValueRef::TopLevel { fqn, .. } => self.codegen_top_level_value_ref(span, fqn),
            hir::ValueRef::Local { id, .. } => {
                let local = self.function_cx.env.get(*id).unwrap_or_else(|| {
                    panic!("codegen_var_ref: HIR verifier accepted an unbound local value")
                });
                let local_ptr = self.local_ptr_for_use(span, local, "load_local_slot")?;

                match local.ty {
                    CgTy::Unit => Ok(CgValue::unit()),
                    CgTy::Never => Ok(CgValue::never()),
                    CgTy::Bool => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_bool",
                            )?
                            .into_int_value();
                        Ok(CgValue::bool(raw))
                    }
                    CgTy::Float64 | CgTy::Float32 => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_float",
                            )?
                            .into_float_value();
                        Ok(CgValue::float(raw, local.ty))
                    }
                    CgTy::Int(int_ty) => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_int",
                            )?
                            .into_int_value();
                        Ok(CgValue::int(raw, int_ty))
                    }
                    CgTy::String => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_str",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::String,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Ref => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_ref",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::Ref,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Tuple(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_tuple",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Struct(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_struct",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Enum(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_enum",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_struct_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Struct(struct_ty)) = self.try_cg_ty_of_type_id(ty) else {
            panic!("codegen_struct_lit: typecheck accepted non-struct struct literal type");
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            panic!("codegen_struct_lit: typecheck accepted struct literal without nominal schema");
        };

        let layout_key = self.nominal_layout_key(nominal);
        let layout = self.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!("codegen_struct_lit: typecheck accepted struct literal without layout")
        });

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut deferred_fields: Vec<(u32, String, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());

        for (idx, field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|f| f.name == field.name) else {
                // User-facing struct literal field coverage is owned by typecheck.
                unreachable!(
                    "typecheck must reject struct literals missing required fields before LLVM codegen"
                );
            };

            let field_cg =
                self.cg_ty_of_layout_field(init.span, field.ty, field.ty_fqn.as_deref())?;

            // 重要：struct 字段 initializer 需要以字段类型作为 expected context。
            //
            // 例如：`Wrap { e: B(7) }` 中的 `B(7)` 是 enum variant ctor call：
            // - 若缺少 expected enum type，后端无法决定该 ctor 对应哪个 enum 的表示；
            // - 这里把 `field_cg` 作为 expected 传入，可与 `val x: E = B(7)` 的路径保持一致。
            let init_v = self.codegen_expr_in_expected_context(&init.value, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if init_v.ty != field_cg {
                self.coerce_value(init.value.span, init_v, field_cg)?
            } else {
                init_v
            };

            let deferred = self.defer_gc_sensitive_cg_value(
                init.value.span,
                &format!("struct_field_{idx}"),
                coerced,
            )?;

            // T0119: For `@CLayout(packed = N)` with N > 1, use the remapped LLVM element index.
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, field.name.clone(), init.value.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, field_span, deferred)) in
            deferred_fields.into_iter().enumerate()
        {
            let materialized = self.materialize_deferred_cg_value(
                field_span,
                &format!("struct_field_reload_{idx}"),
                deferred,
            )?;
            let raw = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_struct_lit: typecheck accepted valueless struct field")
                }),
            };

            let name = format!("insert_{field_name}");
            agg = self.builder.build_insert_value(agg, raw, llvm_idx, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Struct(struct_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_tuple_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_ty)) = self.try_cg_ty_of_type_id(ty) else {
            panic!("codegen_tuple_lit: typecheck accepted non-tuple tuple literal type");
        };

        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty.inner())
        else {
            panic!("codegen_tuple_lit: typecheck accepted tuple literal without tuple schema");
        };

        if element_tys.len() != elements.len() {
            panic!("codegen_tuple_lit: typecheck accepted tuple literal arity drift");
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (elem_expr, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self.try_cg_ty_of_type_id(*elem_ty).unwrap_or_else(|| {
                panic!("codegen_tuple_lit: typecheck accepted unsupported tuple element type")
            });

            // tuple 元素 initializer 也需要带 expected context：
            // 否则 `({ 11 }, 4)` 这类包含 closure literal 的 tuple 会在元素 codegen 时
            // 落回“无 expected function type”的通用 `expression kind` unsupported。
            let elem_v = self.codegen_expr_in_expected_context(elem_expr, Some(elem_cg))?;
            let coerced = self.coerce_value(elem_expr.span, elem_v, elem_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                elem_expr.span,
                &format!("tuple_elem_{idx}"),
                coerced,
            )?;
            deferred_elements.push((idx, elem_expr.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_tuple_lit: typecheck accepted valueless tuple element")
                }),
            };

            let name = format!("insert_elem_{idx}");
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(hir::MemberRef::Value { fqn, .. }) => {
                // T1311：`TypeName.NestedObject` / `Obj.NestedObject` 的"object 值"访问。
                if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton) {
                    return self.codegen_object_value_access(member.span, fqn);
                }

                // T0828：`object` / `companion object` 静态成员访问（backing field 读取）。
                if self.lookup_object_property_by_fqn(fqn).is_some() {
                    return self.codegen_object_property_access(member.span, fqn);
                }

                // `EnumName.Variant`（unit variant）：`RuntimeError.NullAssertionFailed` 等。
                if let Some(v) =
                    self.try_codegen_qualified_enum_unit_variant_value(member.span, fqn)?
                {
                    return Ok(v);
                }

                // 优先使用“当前表达式语境下最精确的 receiver 类型”：
                // - smart-cast / branch narrowing 会把 `receiver.ty` 收窄到比声明更具体的类型；
                // - 普通局部变量若仍只有 `Any` / `Param`，再回退到 env 里保存的原始 `hir_ty`。
                let receiver_hir_ty = self
                    .resolve_expr_concrete_type(receiver)
                    .unwrap_or(receiver.ty);

                // T1312：class 实例字段访问（`this.x` / `obj.x`）。
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
                {
                    if field_cg == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                    let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                    let raw = self.expect_cg_value(recv, "class field receiver");
                    let obj_ptr = self.expect_pointer_value(raw, "class field receiver");

                    let field_ptr =
                        self.codegen_class_field_ptr(member.span, &class, obj_ptr, field_idx)?;
                    let llvm_ty = self.llvm_basic_type_of(member.span, field_cg)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, field_ptr, "load_class_field")?;
                    return self.cg_value_from_loaded(member.span, field_cg, loaded);
                }

                // 优先路径：`localStruct.field` —— 用 GEP 从 alloca slot 取字段（更贴近后续可变字段语义）。
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
                    && let Some(local) = self.function_cx.env.get(*id)
                    && let CgTy::Struct(struct_ty) = local.ty
                {
                    let (field_idx, field_ty) =
                        self.lookup_struct_field(struct_ty, fqn, member.span)?;
                    if field_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let local_ptr = self.local_ptr_for_use(member.span, local, "field_base_ptr")?;
                    let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                    let field_ptr = self.builder.build_struct_gep(
                        llvm_struct_ty,
                        local_ptr,
                        field_idx,
                        "field_gep",
                    )?;
                    let llvm_field_ty = self.llvm_basic_type_of(member.span, field_ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_field_ty, field_ptr, "load_field")?;
                    // `@CLayout(packed = N)`：字段地址可能是非自然对齐的，需要把 load
                    // alignment 降到 `min(field_natural_align, N)` 以避免 UB。
                    if let Some(pack_n) = self.struct_clayout(struct_ty).and_then(|c| c.packed)
                        && let Some(inst) = loaded.as_instruction_value()
                    {
                        let natural = self.target_data.get_abi_alignment(&llvm_field_ty);
                        let effective = std::cmp::min(natural, pack_n);
                        inst.set_alignment(effective)?;
                    }
                    return self.cg_value_from_loaded(member.span, field_ty, loaded);
                }

                // fallback：先把 receiver 降到值，再用 extractvalue 取字段。
                let recv = self.codegen_expr(receiver)?;
                let CgTy::Struct(struct_ty) = recv.ty else {
                    panic!(
                        "codegen_member_access_expr: typecheck accepted non-struct receiver for value member `{}`",
                        member.name
                    );
                };
                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                if field_ty == CgTy::Unit {
                    return Ok(CgValue::unit());
                }

                let raw = self.expect_cg_value(recv, "struct member access receiver");
                let struct_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(struct_v, field_idx, "extract_field")?;
                return self.cg_value_from_loaded(member.span, field_ty, extracted);
            }
            Some(_) => {
                panic!(
                    "codegen_member_access_expr: typecheck accepted non-value member access target"
                );
            }
            None => {}
        }

        // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
        let Some(elem_idx) = parse_tuple_member_index(&member.name) else {
            panic!(
                "codegen_member_access_expr: typecheck accepted unresolved member access target `{}`",
                member.name
            );
        };

        // 优先路径：`localTuple._0` —— 用 GEP 从 alloca slot 取元素。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
            && let Some(local) = self.function_cx.env.get(*id)
            && let CgTy::Tuple(tuple_ty) = local.ty
        {
            let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
            if elem_ty == CgTy::Unit {
                return Ok(CgValue::unit());
            }

            let local_ptr = self.local_ptr_for_use(member.span, local, "tuple_base_ptr")?;
            let llvm_tuple_ty = self.llvm_tuple_type(member.span, tuple_ty)?;
            let elem_ptr = self.builder.build_struct_gep(
                llvm_tuple_ty,
                local_ptr,
                elem_idx,
                "tuple_elem_gep",
            )?;
            let llvm_elem_ty = self.llvm_basic_type_of(member.span, elem_ty)?;
            let loaded = self
                .builder
                .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
            return self.cg_value_from_loaded(member.span, elem_ty, loaded);
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取元素。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Tuple(tuple_ty) = recv.ty else {
            panic!(
                "codegen_member_access_expr: typecheck accepted non-tuple receiver for tuple member `{}`",
                member.name
            );
        };

        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = self.expect_cg_value(recv, "tuple member access receiver");
        let tuple_v = raw.into_struct_value();
        let extracted =
            self.builder
                .build_extract_value(tuple_v, elem_idx, "extract_tuple_elem")?;
        self.cg_value_from_loaded(member.span, elem_ty, extracted)
    }

    pub(in crate::llvm::codegen) fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(raw.into_float_value(), ty),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Tuple(tuple_ty) => CgValue {
                ty: CgTy::Tuple(tuple_ty),
                value: Some(raw),
            },
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
            CgTy::Enum(enum_ty) => CgValue {
                ty: CgTy::Enum(enum_ty),
                value: Some(raw),
            },
            CgTy::Never => CgValue::never(),
        })
    }
}
