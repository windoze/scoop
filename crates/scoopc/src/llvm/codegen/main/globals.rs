//! Class field place deferral and global storage decl: extern globals, top-level vars.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn defer_class_field_place(
        &mut self,
        receiver: &hir::Expr,
        member_span: crate::span::Span,
        field_fqn: &str,
        receiver_hir_ty: TypeId,
        name_prefix: &str,
    ) -> Result<Option<DeferredClassFieldPlace<'ctx>>, LlvmEmitError> {
        let Some((class, field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, member_span, Some(receiver_hir_ty))?
        else {
            return Ok(None);
        };
        let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
            panic!("defer_class_field_place: verifier accepted class field index drift")
        });
        let writable = field.mutable;
        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
        let raw = self.expect_cg_value(recv, "class field receiver");
        let obj_ptr = self.expect_pointer_value(raw, "class field receiver");

        Ok(Some(DeferredClassFieldPlace {
            class,
            field_idx,
            field_cg,
            writable,
            receiver: self.defer_gc_ref_pointer(
                receiver.span,
                &format!("{name_prefix}_receiver"),
                obj_ptr,
            )?,
        }))
    }

    pub(in crate::llvm::codegen) fn reload_deferred_class_field_place_ptr(
        &mut self,
        at: crate::span::Span,
        place: &DeferredClassFieldPlace<'ctx>,
        name_prefix: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let receiver = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{name_prefix}_receiver_reload"),
            &place.receiver,
        )?;
        self.codegen_class_field_ptr(at, &place.class, receiver, place.field_idx)
    }

    pub(in crate::llvm::codegen) fn declare_top_level_var_global(
        &mut self,
        v: &hir::TopLevelVar,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let name = private_top_level_var_global_name(&self.stable_top_level_var_key(&v.fqn));
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(existing);
        }

        let cg_ty = self.expect_cg_ty_of(v.ty, "top-level var global type");

        let llvm_ty = self.llvm_basic_type_of(v.span, cg_ty)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        if v.storage == hir::TopLevelVarStorage::ThreadLocal {
            gv.set_thread_local(true);
        }

        let saved_source_id = self.current_source_id;
        self.current_source_id = self.source_id_for_path(v.source_path.as_path(), v.span)?;
        let init = self.const_initializer_for_top_level_var(v, cg_ty, llvm_ty);
        self.current_source_id = saved_source_id;
        let init = init?;
        gv.set_initializer(&init);

        // `@CLayout(aligned = N)`：对显式对齐的值类型，在全局存储上透传 alignment。
        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }
        Ok(gv)
    }

    pub(in crate::llvm::codegen) fn declare_extern_global(
        &mut self,
        global: &hir::ExternGlobal,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        self.declare_extern_global_storage(
            global.span,
            global.ty,
            &global.symbol,
            global.linkage,
            global.storage,
            global.initializer_absent,
        )
    }

    pub(in crate::llvm::codegen) fn declare_mir_extern_global(
        &mut self,
        global: &crate::mir::ExternGlobalRoot,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        self.declare_extern_global_storage(
            global.span,
            global.ty,
            &global.symbol,
            global.linkage,
            global.storage,
            global.initializer_absent,
        )
    }

    pub(in crate::llvm::codegen) fn declare_extern_global_storage(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        symbol: &str,
        linkage: hir::ExternGlobalLinkage,
        storage: hir::TopLevelVarStorage,
        initializer_absent: bool,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        if !initializer_absent {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global initializer contract",
                at: span.into(),
            });
        }
        let cg_ty = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global type",
                at: span.into(),
            })?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let gv = self
            .module
            .get_global(symbol)
            .unwrap_or_else(|| self.module.add_global(llvm_ty, None, symbol));
        match linkage {
            hir::ExternGlobalLinkage::External => gv.set_linkage(Linkage::External),
        }
        gv.set_thread_local(storage == hir::TopLevelVarStorage::ThreadLocal);

        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }

        Ok(gv)
    }

    pub(in crate::llvm::codegen) fn const_initializer_for_top_level_var(
        &mut self,
        v: &hir::TopLevelVar,
        cg_ty: CgTy,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let Some(init) = v.init.as_ref() else {
            return Ok(self.zero_initializer_for_basic_type(llvm_ty));
        };

        Ok(match cg_ty {
            CgTy::Unit | CgTy::Never => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool => {
                let value = self.const_eval_bool_expr(init).unwrap_or_else(|| {
                    panic!("const_initializer_for_top_level_var: verifier accepted non-const Bool initializer")
                });
                self.context
                    .bool_type()
                    .const_int(value as u64, false)
                    .into()
            }
            CgTy::Int(int_ty) => {
                let bits = self.const_eval_int_expr_bits(init, int_ty)?.unwrap_or_else(|| {
                    panic!("const_initializer_for_top_level_var: verifier accepted non-const Int initializer")
                });
                let value = mask_to_bits(bits, int_ty.bits) as u64;
                self.int_type(int_ty).const_int(value, false).into()
            }
            CgTy::Float64 | CgTy::Float32 => {
                self.const_eval_float_expr(init, cg_ty).unwrap_or_else(|| {
                    panic!("const_initializer_for_top_level_var: verifier accepted non-const Float initializer")
                })
            }
            // 早期阶段：仅支持"静态全零初始化"；更复杂的值类型常量构造留给后续任务补齐。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                panic!("const_initializer_for_top_level_var: verifier accepted aggregate top-level var initializer");
            }
            CgTy::String | CgTy::Ref => {
                panic!("const_initializer_for_top_level_var: verifier accepted GC top-level var initializer");
            }
        })
    }
}
