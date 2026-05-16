//! Itable contains / float-int binary / shift / compare lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// 只要其中任意一个 target type id 与 `target_type_id` 相等，就判定为 true。
    pub(in crate::llvm::codegen) fn codegen_itable_contains_runtime_type_id(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_type_id: u64,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // obj 指向对象头起始地址：先把它 cast 为 `ScoopGcObjectHeader*`。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(obj, header_ptr_ty, "isa_iface_hdr_ptr")?;

        // header.type_desc : i8*
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_iface_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_iface_load_type_desc")?
            .into_pointer_value();

        // type_desc.itable : i8*
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr =
            self.builder
                .build_pointer_cast(type_desc_i8, desc_ptr_ty, "isa_iface_type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "isa_iface_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "isa_iface_load_itable")?
            .into_pointer_value();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        // itable == NULL -> false
        let itable_is_null = self
            .builder
            .build_is_null(itable_i8, "isa_iface_itable_is_null")?;
        let null_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_null");
        let lookup_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_lookup");
        let done_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // lookup：扫描 entries[idx].runtime_match_type_ids
        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr =
            self.builder
                .build_pointer_cast(itable_i8, itable_ptr_ty, "isa_iface_itable_ptr")?;

        let len_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 0, "isa_iface_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "isa_iface_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "isa_iface_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base = self.builder.build_pointer_cast(
            entries_field_ptr,
            entry_ptr_ty,
            "isa_iface_entries",
        )?;

        let loop_bb = self.context.append_basic_block(func, "isa_iface_loop");
        let body_bb = self.context.append_basic_block(func, "isa_iface_body");
        let hit_bb = self.context.append_basic_block(func, "isa_iface_hit");
        let miss_bb = self.context.append_basic_block(func, "isa_iface_miss");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "isa_iface_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "isa_iface_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, body_bb, done_bb)?;

        // body：线性扫描当前 entry 的 runtime_match_type_ids。
        self.builder.position_at_end(body_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "isa_iface_entry_ptr",
            )?
        };
        let match_len_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 1, "isa_iface_match_len_gep")?;
        let match_len_i32 = self
            .builder
            .build_load(i32_ty, match_len_ptr, "isa_iface_match_len")?
            .into_int_value();
        let match_ids_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 3, "isa_iface_match_ids_gep")?;
        let match_ids_i8 = self
            .builder
            .build_load(i8_ptr_ty, match_ids_ptr, "isa_iface_match_ids")?
            .into_pointer_value();

        let entry_match_ids_null = self
            .builder
            .build_is_null(match_ids_i8, "isa_iface_match_ids_is_null")?;
        let entry_match_len_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            match_len_i32,
            i32_ty.const_zero(),
            "isa_iface_match_len_is_zero",
        )?;
        let entry_match_empty = self.builder.build_or(
            entry_match_ids_null,
            entry_match_len_zero,
            "isa_iface_match_empty",
        )?;
        let entry_match_lookup_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_lookup");
        self.builder
            .build_conditional_branch(entry_match_empty, miss_bb, entry_match_lookup_bb)?;

        self.builder.position_at_end(entry_match_lookup_bb);
        let match_ids_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let match_ids_base = self.builder.build_pointer_cast(
            match_ids_i8,
            match_ids_ptr_ty,
            "isa_iface_match_ids_base",
        )?;
        let match_loop_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_loop");
        let match_body_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_body");
        let match_done_miss_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_done_miss");
        self.builder.build_unconditional_branch(match_loop_bb)?;

        self.builder.position_at_end(match_loop_bb);
        let match_idx_phi = self.builder.build_phi(i32_ty, "isa_iface_match_idx")?;
        match_idx_phi.add_incoming(&[(&i32_ty.const_zero(), entry_match_lookup_bb)]);
        let match_idx_i32 = match_idx_phi.as_basic_value().into_int_value();
        let match_cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            match_idx_i32,
            match_len_i32,
            "isa_iface_match_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(match_cond, match_body_bb, match_done_miss_bb)?;

        self.builder.position_at_end(match_body_bb);
        let match_slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i64_ty,
                match_ids_base,
                &[match_idx_i32],
                "isa_iface_match_slot_ptr",
            )?
        };
        let match_id_i64 = self
            .builder
            .build_load(i64_ty, match_slot_ptr, "isa_iface_match_id")?
            .into_int_value();
        let target_id = i64_ty.const_int(target_type_id, false);
        let ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            match_id_i64,
            target_id,
            "isa_iface_match_id_eq",
        )?;
        let match_next_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_next");
        self.builder
            .build_conditional_branch(ok, hit_bb, match_next_bb)?;

        self.builder.position_at_end(match_next_bb);
        let match_next = self.builder.build_int_add(
            match_idx_i32,
            i32_ty.const_int(1, false),
            "isa_iface_match_idx_next",
        )?;
        match_idx_phi.add_incoming(&[(&match_next, match_next_bb)]);
        self.builder.build_unconditional_branch(match_loop_bb)?;

        self.builder.position_at_end(match_done_miss_bb);
        self.builder.build_unconditional_branch(miss_bb)?;

        // miss：idx++ 继续 loop
        self.builder.position_at_end(miss_bb);
        let next = self.builder.build_int_add(
            idx_i32,
            i32_ty.const_int(1, false),
            "isa_iface_idx_next",
        )?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：直接 done
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 false/true
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_iface_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    pub(in crate::llvm::codegen) fn codegen_float_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, out_ty)?;
        let r = self.cast_float(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_float_add(l, r, "fadd")?,
            ast::BinaryOp::Sub => self.builder.build_float_sub(l, r, "fsub")?,
            ast::BinaryOp::Mul => self.builder.build_float_mul(l, r, "fmul")?,
            ast::BinaryOp::Div => self.builder.build_float_div(l, r, "fdiv")?,
            ast::BinaryOp::Rem => self.builder.build_float_rem(l, r, "frem")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::float(out, out_ty))
    }

    pub(in crate::llvm::codegen) fn codegen_int_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "integer binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, out_ty)?;
        let r = self.cast_int(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            ast::BinaryOp::Sub => self.builder.build_int_sub(l, r, "sub")?,
            ast::BinaryOp::Mul => self.builder.build_int_mul(l, r, "mul")?,
            ast::BinaryOp::Div => {
                if out_ty.signed {
                    self.builder.build_int_signed_div(l, r, "sdiv")?
                } else {
                    self.builder.build_int_unsigned_div(l, r, "udiv")?
                }
            }
            ast::BinaryOp::Rem => {
                if out_ty.signed {
                    self.builder.build_int_signed_rem(l, r, "srem")?
                } else {
                    self.builder.build_int_unsigned_rem(l, r, "urem")?
                }
            }
            ast::BinaryOp::BitAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::BitXor => self.builder.build_xor(l, r, "xor")?,
            ast::BinaryOp::BitOr => self.builder.build_or(l, r, "or")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, out_ty))
    }

    pub(in crate::llvm::codegen) fn codegen_shift(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (lhs_value, lhs_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift lhs type",
                    at: span.into(),
                })?;

        let rhs_value =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift rhs type",
                    at: span.into(),
                })?;

        let shift_count = self.mask_shift_count(lhs_ty, rhs_value.0)?;

        let out = match op {
            ast::BinaryOp::Shl => self
                .builder
                .build_left_shift(lhs_value, shift_count, "shl")?,
            ast::BinaryOp::Shr => {
                self.builder
                    .build_right_shift(lhs_value, shift_count, lhs_ty.signed, "shr")?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, lhs_ty))
    }

    pub(in crate::llvm::codegen) fn mask_shift_count(
        &mut self,
        lhs_ty: IntTy,
        rhs: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let lhs_bits = lhs_ty.bits;
        let lhs_int = self.int_type(lhs_ty);

        // 1) 截断为 lhs 的位宽（只取低位，后续再 mask）。
        let rhs_trunc = self
            .builder
            .build_int_truncate(rhs, lhs_int, "shift_rhs_trunc")?;

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对"超范围 shift"的 UB。
        let mask = lhs_int.const_int((lhs_bits.saturating_sub(1)) as u64, false);
        Ok(self.builder.build_and(rhs_trunc, mask, "shift_masked")?)
    }

    pub(in crate::llvm::codegen) fn codegen_int_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison rhs",
                    at: span.into(),
                })?;

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match (op, int_ty.signed) {
            (ast::BinaryOp::Lt, true) => IntPredicate::SLT,
            (ast::BinaryOp::Lt, false) => IntPredicate::ULT,
            (ast::BinaryOp::Le, true) => IntPredicate::SLE,
            (ast::BinaryOp::Le, false) => IntPredicate::ULE,
            (ast::BinaryOp::Gt, true) => IntPredicate::SGT,
            (ast::BinaryOp::Gt, false) => IntPredicate::UGT,
            (ast::BinaryOp::Ge, true) => IntPredicate::SGE,
            (ast::BinaryOp::Ge, false) => IntPredicate::UGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp")?,
        ))
    }

    pub(in crate::llvm::codegen) fn codegen_float_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison rhs",
                    at: span.into(),
                })?;

        let float_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, float_ty)?;
        let r = self.cast_float(r_raw, r_ty, float_ty)?;

        let pred = match op {
            ast::BinaryOp::Lt => FloatPredicate::OLT,
            ast::BinaryOp::Le => FloatPredicate::OLE,
            ast::BinaryOp::Gt => FloatPredicate::OGT,
            ast::BinaryOp::Ge => FloatPredicate::OGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_float_compare(pred, l, r, "fcmp")?,
        ))
    }
}
