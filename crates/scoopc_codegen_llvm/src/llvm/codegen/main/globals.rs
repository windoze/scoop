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
        let root = self
            .expect_lir_global_root_kind(
                &v.fqn,
                LirGlobalRootKind::TopLevelMutableVar,
                "declare_top_level_var_global",
            )
            .clone();
        self.declare_lir_top_level_var_global(&root)
    }

    pub(in crate::llvm::codegen) fn declare_lir_top_level_var_global(
        &mut self,
        root: &LirGlobalRootFacts,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let name = private_top_level_var_global_name(&self.stable_def_key_for_lir_global_root(
            root,
            StableDefNamespace::Value,
            "top_level_var",
        ));
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(existing);
        }

        let root_ty = self.lir_global_root_ty(root, "top-level var global type");
        let cg_ty = self.cg_ty_of_type_id(root_ty, "top-level var global type");

        let span = crate::span::Span::synthetic_prelude();
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        if root.storage == Some(LirGlobalStoragePolicy::ThreadLocal) {
            gv.set_thread_local(true);
        }

        gv.set_initializer(&self.zero_initializer_for_basic_type(llvm_ty));

        // `@CLayout(aligned = N)`：对显式对齐的值类型，在全局存储上透传 alignment。
        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }
        Ok(gv)
    }

    pub(in crate::llvm::codegen) fn emit_top_level_var_eager_initializer(
        &mut self,
        v: &hir::TopLevelVar,
    ) -> Result<(), LlvmEmitError> {
        let Some(init) = v.init.as_ref() else {
            return Ok(());
        };
        let cg_ty = self.cg_ty_of_type_id(v.ty, "top-level var eager init type");
        if cg_ty == CgTy::Unit {
            return Ok(());
        }

        let saved_source_id = self.current_source_id;
        self.current_source_id = self.source_id_for_path(v.source_path.as_path(), v.span)?;
        let result = self.emit_top_level_var_eager_initializer_body(v, init, cg_ty);
        self.current_source_id = saved_source_id;
        result
    }

    fn emit_top_level_var_eager_initializer_body(
        &mut self,
        v: &hir::TopLevelVar,
        init: &hir::Expr,
        cg_ty: CgTy,
    ) -> Result<(), LlvmEmitError> {
        let global = self.declare_top_level_var_global(v)?;
        let init_value = self.codegen_initializer_expr(init, cg_ty, v.ty)?;
        let _stored =
            self.store_local_value(init.span, global.as_pointer_value(), cg_ty, init_value)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn declare_lir_extern_global(
        &mut self,
        root: &LirGlobalRootFacts,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let extern_global = root.extern_global.as_ref().unwrap_or_else(|| {
            panic!(
                "declare_lir_extern_global: LIR facts extern root `{}` is missing extern contract",
                root.root.as_str()
            )
        });
        self.declare_extern_global_storage(
            crate::span::Span::synthetic_prelude(),
            self.lir_global_root_ty(root, "extern global storage type"),
            &extern_global.symbol,
            extern_global.linkage,
            self.lir_global_storage_policy_as_hir(root, "extern global storage policy"),
            extern_global.initializer_absent,
        )
    }

    pub(in crate::llvm::codegen) fn declare_extern_global_storage(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        symbol: &str,
        linkage: LirExternGlobalLinkage,
        storage: hir::TopLevelVarStorage,
        initializer_absent: bool,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        if !initializer_absent {
            panic!("declare_extern_global_storage: verifier accepted extern global initializer");
        }
        let cg_ty = self.cg_ty_of_type_id(ty, "extern global storage type");
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let gv = self
            .module
            .get_global(symbol)
            .unwrap_or_else(|| self.module.add_global(llvm_ty, None, symbol));
        match linkage {
            LirExternGlobalLinkage::External => gv.set_linkage(Linkage::External),
        }
        gv.set_thread_local(storage == hir::TopLevelVarStorage::ThreadLocal);

        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }

        Ok(gv)
    }
}
