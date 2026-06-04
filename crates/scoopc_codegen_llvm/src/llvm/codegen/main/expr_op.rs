//! Unary, binary, operator overload, type check, cast as / asq, ref instanceof.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Resolve the effective CgTy for an expression, preferring concrete type
    /// sources over the often-widened HIR `expr.ty`.
    pub(in crate::llvm::codegen) fn resolve_expr_cg_ty(&self, expr: &hir::Expr) -> Option<CgTy> {
        // Locals keep their exact lowered codegen type in the environment, so
        // prefer that before reconstructing from a TypeId.
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind
            && let Some(local) = self.function_cx.env.get(*id)
        {
            return Some(local.ty);
        }

        // HIR lowering writes all VarRef expressions as `Any`, including
        // top-level refs. Reuse the broader concrete-type resolver so builtin
        // call sites like `Continuation.resume(payload)` do not regress when
        // the payload comes from a top-level typed binding.
        if let Some(concrete_ty) = self.resolve_expr_concrete_type(expr) {
            return self.try_cg_ty_of_type_id(concrete_ty);
        }

        self.try_cg_ty_of_type_id(expr.ty)
    }

    pub(in crate::llvm::codegen) fn is_unsuffixed_float64_literal(expr: &hir::Expr) -> bool {
        matches!(
            expr.kind,
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_))
        )
    }

    pub(in crate::llvm::codegen) fn unify_float_cg_types(
        &self,
        lhs: &hir::Expr,
        lhs_ty: CgTy,
        rhs: &hir::Expr,
        rhs_ty: CgTy,
    ) -> Option<CgTy> {
        match (lhs_ty, rhs_ty) {
            (CgTy::Float64, CgTy::Float64) => Some(CgTy::Float64),
            (CgTy::Float32, CgTy::Float32) => Some(CgTy::Float32),
            (CgTy::Float64, CgTy::Float32) if Self::is_unsuffixed_float64_literal(lhs) => {
                Some(CgTy::Float32)
            }
            (CgTy::Float32, CgTy::Float64) if Self::is_unsuffixed_float64_literal(rhs) => {
                Some(CgTy::Float32)
            }
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn codegen_type_check_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::TypeCheckOp,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段只实现 ref→ref 的运行期检查（T1509b）：
        // - class：沿 type descriptor parent 链查找；
        // - interface：扫描 itable 是否包含 interface_id。
        //
        // 说明：typecheck 阶段对 `is/!is` 的静态约束仍偏弱（只保证 type lowering），
        // 因此 codegen 侧需要做"不可支持场景"的防御式报错，避免 silent miscompile。
        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                panic!("codegen_type_check_expr: typecheck gate accepted non-runtime-ref operand");
            }
        };
        let Some(raw) = v.value else {
            panic!("codegen_type_check_expr: typecheck gate accepted valueless operand");
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            panic!("codegen_type_check_expr: typecheck gate accepted non-pointer runtime operand");
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;
        let out = match op {
            ast::TypeCheckOp::Is => is_ok,
            ast::TypeCheckOp::NotIs => self.builder.build_not(is_ok, "typecheck_not")?,
        };
        Ok(CgValue::bool(out))
    }

    pub(in crate::llvm::codegen) fn codegen_cast_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::CastOp,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::CastOp::As => self.codegen_cast_as_expr(span, expr, target_ty),
            ast::CastOp::AsQ => self.codegen_cast_asq_expr(span, expr, target_ty, out_ty),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_cast_as_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_cg = self.cg_ty_of_type_id(target_ty, "`as` cast target type");
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                panic!("codegen_cast_as_expr: typecheck gate accepted non-runtime-ref target")
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                panic!("codegen_cast_as_expr: typecheck gate accepted non-runtime-ref operand")
            }
        };
        let obj_ptr = self.expect_cg_pointer(v, "`as` cast operand");

        // 运行期检查：为避免在 obj=NULL 时解引用对象头，先对 NULL 做 fail 处理。
        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

        let func = self.expect_current_function("checked cast branch blocks");

        let ok_bb = self.context.append_basic_block(func, "cast_ok");
        let fail_bb = self.context.append_basic_block(func, "cast_fail");
        let merge_bb = self.context.append_basic_block(func, "cast_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "cast_ptr")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail ---
        self.builder.position_at_end(fail_bb);
        self.emit_panic_message(span, "class cast failed")?;
        self.builder.build_unreachable()?;

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(target_ptr_ty, "cast_value")?;
        phi.add_incoming(&[(&casted_ptr, ok_bb)]);
        let out_ptr = phi.as_basic_value().into_pointer_value();

        Ok(CgValue {
            ty: target_cg,
            value: Some(out_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_cast_asq_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `as?` 的结果类型应为 `Option<target_ty>`（或等价 nullable sugar）。
        let out_cg = self.cg_ty_of_type_id(out_ty, "`as?` cast result type");
        let CgTy::Enum(option_ty) = out_cg else {
            panic!("codegen_cast_asq_expr: typecheck gate accepted non-Option `as?` result type");
        };

        let target_cg = self.cg_ty_of_type_id(target_ty, "`as?` cast target type");
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                panic!(
                    "codegen_cast_asq_expr: typecheck gate accepted non-runtime-ref `as?` target"
                );
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                panic!(
                    "codegen_cast_asq_expr: typecheck gate accepted non-runtime-ref `as?` operand"
                );
            }
        };
        let Some(raw) = v.value else {
            panic!("codegen_cast_asq_expr: typecheck gate accepted valueless `as?` operand");
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            panic!("codegen_cast_asq_expr: typecheck gate accepted non-pointer `as?` operand");
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

        let func = self.expect_current_function("nullable cast branch blocks");

        let ok_bb = self.context.append_basic_block(func, "asq_ok");
        let fail_bb = self.context.append_basic_block(func, "asq_fail");
        let merge_bb = self.context.append_basic_block(func, "asq_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok：Some(casted) ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "asq_cast_ptr")?;
        let casted = CgValue {
            ty: target_cg,
            value: Some(casted_ptr.into()),
        };
        let payload = self.coerce_enum_payload(span, casted, target_cg)?;
        let some_v = self.build_enum_value(span, option_ty, 0, payload)?;
        let some_raw = some_v.value.unwrap_or_else(|| {
            panic!("codegen_cast_asq_expr: verified Option Some produced no value")
        });
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail：None ---
        self.builder.position_at_end(fail_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.unwrap_or_else(|| {
            panic!("codegen_cast_asq_expr: verified Option None produced no value")
        });
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let llvm_option_ty = self.llvm_enum_value_type(span, option_ty)?;
        let phi = self.builder.build_phi(llvm_option_ty, "asq_value")?;
        phi.add_incoming(&[(&some_raw, ok_bb), (&none_raw, fail_bb)]);
        let out_raw = phi.as_basic_value();

        Ok(CgValue {
            ty: CgTy::Enum(option_ty),
            value: Some(out_raw),
        })
    }

    /// 运行期类型检查：判断 `obj` 是否为 `target_ty` 的实例。
    ///
    /// 约定（v0，T1509b）：
    /// - 若 `obj == NULL`：返回 false（避免解引用 NULL）；
    /// - `Any`：只要非 NULL 即为 true（不依赖 type_desc）；
    /// - class：沿 `type_desc.parent_type_desc` 向上查找；
    /// - interface：扫描 itable entries 的 runtime target match 集。
    pub(in crate::llvm::codegen) fn codegen_ref_is_instance_of(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let obj_is_null = self.builder.build_is_null(obj, "isa_obj_is_null")?;

        // fast path：`x is Any` 只需要判空。
        if matches!(self.types.kind(target_ty), TypeKind::Ref(RefTypeKind::Any)) {
            return Ok(self.builder.build_not(obj_is_null, "isa_any_nonnull")?);
        }

        // 对其它 target：obj 为 NULL 时直接 false，避免解引用对象头。
        let func = self.expect_current_function("runtime type check null split");

        let null_bb = self.context.append_basic_block(func, "isa_obj_null");
        let nonnull_bb = self.context.append_basic_block(func, "isa_obj_nonnull");
        let done_bb = self.context.append_basic_block(func, "isa_done");
        self.builder
            .build_conditional_branch(obj_is_null, null_bb, nonnull_bb)?;

        // null -> done(false)
        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // nonnull -> 计算真实检查 -> done
        self.builder.position_at_end(nonnull_bb);
        let inner_ok = self.codegen_ref_is_instance_of_nonnull(at, obj, target_ty)?;
        let after_check_bb = self.expect_insert_block("runtime type check tail block");
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_result")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&inner_ok, after_check_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    pub(in crate::llvm::codegen) fn codegen_ref_is_instance_of_nonnull(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match self.types.kind(target_ty) {
            TypeKind::Ref(RefTypeKind::Any) => Ok(self.context.bool_type().const_int(1, false)),
            TypeKind::Ref(RefTypeKind::String) => {
                let desc = self.get_or_create_string_type_desc_global(at)?;
                let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                self.codegen_type_desc_chain_contains_target(at, obj, target_i8)
            }
            TypeKind::Ref(RefTypeKind::Function(_)) => {
                let desc = self.get_or_create_closure_object_type_desc_global(at, target_ty)?;
                let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                self.codegen_type_desc_chain_contains_target(at, obj, target_i8)
            }
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                // interface：用 itable 中预计算的 runtime target match 集判断是否可赋值到目标实例。
                if self
                    .expect_active_lir_program("codegen_ref_is_instance_of_nonnull")
                    .physical_layout()
                    .interfaces
                    .contains_key(nominal.fqn.as_str())
                {
                    let target_type_id = self.stable_rtti_type_id_for_codegen(
                        target_ty,
                        "interface runtime-match target",
                    )?;
                    return self.codegen_itable_contains_runtime_type_id(at, obj, target_type_id);
                }

                // class：沿 parent 链查找。
                let class_lookup_key = self
                    .types
                    .as_mono(target_ty)
                    .ok()
                    .and_then(|mono_ty| {
                        hir::ClassInstanceKey::from_mono_nominal(self.types, mono_ty)
                    })
                    .filter(|key| self.class_inits.contains_key(key));
                if let Some(class_key) = class_lookup_key {
                    let desc = self.get_or_create_class_type_desc_global(at, &class_key)?;
                    let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                    return self.codegen_type_desc_chain_contains_target(at, obj, target_i8);
                }

                if self.lir_global_root_has_kind(&nominal.fqn, LirGlobalRootKind::ObjectSingleton) {
                    let desc =
                        self.get_or_create_object_singleton_type_desc_global(at, &nominal.fqn)?;
                    let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                    return self.codegen_type_desc_chain_contains_target(at, obj, target_i8);
                }

                panic!(
                    "codegen_ref_is_instance_of_nonnull: typecheck gate accepted unsupported nominal runtime target"
                )
            }
            _ => panic!(
                "codegen_ref_is_instance_of_nonnull: typecheck gate accepted unsupported runtime target type"
            ),
        }
    }

    /// `class` 类型判断：检查 `obj.header.type_desc` 的 parent 链是否包含 `target_desc_i8`。
    pub(in crate::llvm::codegen) fn codegen_type_desc_chain_contains_target(
        &mut self,
        _at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_desc_i8: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // 读取 `header.type_desc`（i8*）。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr = self
            .builder
            .build_pointer_cast(obj, header_ptr_ty, "isa_hdr_ptr")?;
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_type_desc")?
            .into_pointer_value();

        let insert_block = self.expect_insert_block("runtime type descriptor chain");
        let func = self.expect_parent_function(insert_block, "runtime type descriptor chain");

        // while (cur != NULL) { if (cur == target) return true; cur = cur.parent; } return false
        let loop_bb = self.context.append_basic_block(func, "isa_loop");
        let check_bb = self.context.append_basic_block(func, "isa_check");
        let advance_bb = self.context.append_basic_block(func, "isa_advance");
        let hit_bb = self.context.append_basic_block(func, "isa_hit");
        let done_bb = self.context.append_basic_block(func, "isa_done");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let cur_phi = self.builder.build_phi(i8_ptr_ty, "isa_cur")?;
        cur_phi.add_incoming(&[(&type_desc_i8, insert_block)]);
        let cur_i8 = cur_phi.as_basic_value().into_pointer_value();

        let cur_is_null = self.builder.build_is_null(cur_i8, "isa_cur_is_null")?;
        self.builder
            .build_conditional_branch(cur_is_null, done_bb, check_bb)?;

        // check：cur == target ?
        self.builder.position_at_end(check_bb);
        let word_ty = self.int_type(IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        });
        let cur_int = self
            .builder
            .build_ptr_to_int(cur_i8, word_ty, "isa_cur_int")?;
        let target_int =
            self.builder
                .build_ptr_to_int(target_desc_i8, word_ty, "isa_target_int")?;
        let eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, cur_int, target_int, "isa_eq")?;
        self.builder
            .build_conditional_branch(eq, hit_bb, advance_bb)?;

        // advance：cur = cur.parent
        self.builder.position_at_end(advance_bb);
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let cur_desc = self
            .builder
            .build_pointer_cast(cur_i8, desc_ptr_ty, "isa_desc")?;
        let parent_ptr = self
            .builder
            .build_struct_gep(desc_ty, cur_desc, 11, "isa_parent_gep")?;
        let parent_desc = self
            .builder
            .build_load(desc_ptr_ty, parent_ptr, "isa_parent")?
            .into_pointer_value();
        let parent_i8 = self
            .builder
            .build_pointer_cast(parent_desc, i8_ptr_ty, "isa_parent_i8")?;
        cur_phi.add_incoming(&[(&parent_i8, advance_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：return true
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 true/false
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }
}
