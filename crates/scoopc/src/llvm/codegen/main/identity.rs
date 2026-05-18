//! MainCodegen lifecycle (new / fresh_child_codegen) and stable identity: stable def keys, exported ABI symbol reservation, closure stable keys.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn new(shared: &'a CompilationUnitCodegenCx<'a, 'ctx>) -> Self {
        Self {
            shared,
            current_source_id: shared.entry_source_id,
            function_cx: FunctionBodyCodegenCx::default(),
            effect_cx: EffectLoweringCodegenCx::default(),
        }
    }

    /// 统一 nested/wrapper codegen 的构造路径，避免再次手写整套编译单元输入拼装。
    pub(in crate::llvm::codegen) fn fresh_child_codegen(&self) -> Self {
        Self::new(self.shared)
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_current_cone(
        &self,
        namespace: StableDefNamespace,
        owner_path: &str,
        declaration_kind: &str,
    ) -> StableDefKey {
        StableDefKey::new(
            self.stable_cone_key.clone(),
            namespace,
            owner_path,
            declaration_kind,
            None,
        )
    }

    pub(in crate::llvm::codegen) fn stable_top_level_immutable_value_key(
        &self,
        value_fqn: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_current_cone(
            StableDefNamespace::Value,
            value_fqn,
            "top_level_value",
        )
    }

    pub(in crate::llvm::codegen) fn stable_top_level_init_key(
        &self,
        value_fqn: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_current_cone(
            StableDefNamespace::TopLevelInit,
            value_fqn,
            "top_level_init",
        )
    }

    pub(in crate::llvm::codegen) fn stable_top_level_var_key(&self, var_fqn: &str) -> StableDefKey {
        self.stable_def_key_for_current_cone(StableDefNamespace::Value, var_fqn, "top_level_var")
    }

    pub(in crate::llvm::codegen) fn stable_nominal_type_key(
        &self,
        type_fqn: &str,
        declaration_kind: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_current_cone(StableDefNamespace::Type, type_fqn, declaration_kind)
    }

    pub(in crate::llvm::codegen) fn canonical_type_key_text_for_codegen(
        &self,
        ty: TypeId,
        context: &str,
    ) -> Result<String, LlvmEmitError> {
        let types =
            self.codegen_type_store_for_type_id(ty)
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!("{context} 缺少 codegen type store"),
                })?;
        canonical_type_text(types, ty, self.stable_type_param_resolver()).map_err(|err| {
            LlvmEmitError::Frontend {
                message: format!("{context} 无法构造 stable canonical type key: {err}"),
            }
        })
    }

    pub(in crate::llvm::codegen) fn stable_rtti_type_id_for_codegen(
        &self,
        ty: TypeId,
        context: &str,
    ) -> Result<u64, LlvmEmitError> {
        let types =
            self.codegen_type_store_for_type_id(ty)
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!("{context} 缺少 codegen type store"),
                })?;
        stable_rtti_type_id_for_type(types, ty, self.stable_type_param_resolver()).map_err(|err| {
            LlvmEmitError::Frontend {
                message: format!("{context} 无法构造 stable RTTI type id: {err}"),
            }
        })
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_callable_signature(
        &self,
        owner_path: &str,
        declaration_kind: &str,
        callable_ty: TypeId,
        types: &TypeStore,
    ) -> Result<StableDefKey, LlvmEmitError> {
        let signature_key = canonical_callable_signature_key(
            types,
            callable_ty,
            0,
            0,
            0,
            self.stable_type_param_resolver(),
        )
        .map_err(|err| LlvmEmitError::Frontend {
            message: format!(
                "无法为 callable `{owner_path}` 计算 stable exported signature key: {err}"
            ),
        })?;
        Ok(StableDefKey::new(
            self.stable_cone_key.clone(),
            StableDefNamespace::Fun,
            callable_export_readable_path(owner_path),
            declaration_kind,
            Some(signature_key),
        ))
    }

    pub(in crate::llvm::codegen) fn reserve_exported_abi_symbol<K>(
        &self,
        symbol: &str,
        stable_key: &K,
        owner_label: impl Into<String>,
    ) -> Result<(), LlvmEmitError>
    where
        K: StableCanonicalKey + ?Sized,
    {
        reserve_exported_abi_symbol_in_registry(
            &self.shared.shared_caches.exported_abi_symbols,
            symbol,
            stable_key.canonical_text(),
            owner_label.into(),
        )
        .map_err(|message| LlvmEmitError::Frontend { message })
    }

    pub(in crate::llvm::codegen) fn exported_abi_symbol_for_hir_fun(
        &self,
        fun: &hir::FunDecl,
    ) -> Result<String, LlvmEmitError> {
        if fun.fqn == "main" {
            return Ok("main".to_string());
        }
        if let Some(pass_view) = self.materialized_pass_view()
            && let Some(owner) = pass_view.owner_of_callable(&fun.fqn)
            && let Some(stable_key) = pass_view
                .materialized()
                .authoritative_stable_instance_key(owner)
        {
            let symbol = AbiMangler.fun_symbol(&stable_key);
            self.reserve_exported_abi_symbol(
                &symbol,
                &stable_key,
                format!(
                    "source callable `{}` via authoritative instance key",
                    fun.fqn
                ),
            )?;
            return Ok(symbol);
        }
        let stable_key = self.stable_def_key_for_callable_signature(
            &fun.fqn,
            "non_generic_callable",
            fun.ty,
            self.types,
        )?;
        let symbol = AbiMangler.fun_symbol(&stable_key);
        self.reserve_exported_abi_symbol(
            &symbol,
            &stable_key,
            format!("source callable `{}`", fun.fqn),
        )?;
        Ok(symbol)
    }

    pub(in crate::llvm::codegen) fn exported_abi_symbol_for_hir_fun_with_signature_override(
        &self,
        fun: &hir::FunDecl,
        owner_path: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<String, LlvmEmitError> {
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(fun.ty) else {
            std::panic::panic_any("HIR function declarations must carry function types");
        };
        let mut signature_types = self.types.clone();
        let callable_ty = signature_types.ty_function(
            fun_ty.receiver,
            param_tys.to_vec(),
            return_ty,
            fun_ty.effects.clone(),
            fun_ty.effects_closed,
        );
        let param_ty_text = param_tys
            .iter()
            .map(|&ty| signature_types.display(ty).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let return_ty_text = signature_types.display(return_ty).to_string();
        let stable_key = self.stable_def_key_for_callable_signature(
            owner_path,
            "non_generic_callable",
            callable_ty,
            &signature_types,
        )
        .map_err(|err| LlvmEmitError::Frontend {
            message: format!(
                "无法为 callable `{owner_path}` 基于 signature override 计算 stable exported signature key: params=[{param_ty_text}] return={return_ty_text}: {err}"
            ),
        })?;
        let symbol = AbiMangler.fun_symbol(&stable_key);
        self.reserve_exported_abi_symbol(
            &symbol,
            &stable_key,
            format!(
                "source callable `{}` via signature override concrete callable type",
                owner_path
            ),
        )?;
        Ok(symbol)
    }

    pub(in crate::llvm::codegen) fn exported_abi_symbol_for_materialized_fun(
        &self,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
    ) -> Result<String, LlvmEmitError> {
        if mir_fun.fqn == "main" {
            return Ok("main".to_string());
        }
        if let Some(pass_view) = self.materialized_pass_view()
            && let Some(owner) = pass_view.owner_of_callable(&mir_fun.fqn)
            && let Some(stable_key) = pass_view
                .materialized()
                .authoritative_stable_instance_key(owner)
        {
            let symbol = AbiMangler.fun_symbol(&stable_key);
            self.reserve_exported_abi_symbol(
                &symbol,
                &stable_key,
                format!(
                    "materialized callable `{}` via authoritative instance key",
                    mir_fun.fqn
                ),
            )?;
            return Ok(symbol);
        }
        if let Some(hir_fun) = self.hir_fun_for_callable_fqn(&mir_fun.fqn)
            && hir_fun.fqn == mir_fun.fqn
        {
            return self.exported_abi_symbol_for_hir_fun(hir_fun);
        }
        let stable_key = self.stable_def_key_for_callable_signature(
            &mir_fun.fqn,
            "non_generic_callable",
            mir_fun.ty,
            mir_types,
        )?;
        let symbol = AbiMangler.fun_symbol(&stable_key);
        self.reserve_exported_abi_symbol(
            &symbol,
            &stable_key,
            format!("materialized callable `{}`", mir_fun.fqn),
        )?;
        Ok(symbol)
    }

    pub(in crate::llvm::codegen) fn enter_root_callable_identity(
        &mut self,
        callable_fqn: String,
        stable_owner_key: StableDefKey,
    ) {
        self.function_cx.current_callable_fqn = Some(callable_fqn);
        self.function_cx.current_stable_owner_key = Some(stable_owner_key);
        self.function_cx.current_stable_closure_path_prefix = None;
        self.function_cx.next_stable_child_closure_index = 0;
        self.function_cx.stable_closure_paths.clear();
    }

    pub(in crate::llvm::codegen) fn enter_nested_closure_identity(
        &mut self,
        callable_fqn: String,
        stable_owner_key: StableDefKey,
        stable_closure_path: &str,
    ) {
        self.function_cx.current_callable_fqn = Some(callable_fqn);
        self.function_cx.current_stable_owner_key = Some(stable_owner_key);
        self.function_cx.current_stable_closure_path_prefix = Some(stable_closure_path.to_string());
        self.function_cx.next_stable_child_closure_index = 0;
        self.function_cx.stable_closure_paths.clear();
    }

    pub(in crate::llvm::codegen) fn current_stable_owner_key(
        &self,
        _at: crate::span::Span,
        kind: &'static str,
    ) -> Result<StableDefKey, LlvmEmitError> {
        Ok(self
            .function_cx
            .current_stable_owner_key
            .clone()
            .unwrap_or_else(|| std::panic::panic_any(kind)))
    }

    pub(in crate::llvm::codegen) fn next_stable_child_closure_path(
        &mut self,
        closure_id: hir::ClosureId,
    ) -> String {
        if let Some(existing) = self.function_cx.stable_closure_paths.get(&closure_id) {
            return existing.clone();
        }
        let ordinal = self.function_cx.next_stable_child_closure_index;
        self.function_cx.next_stable_child_closure_index += 1;
        let path = match &self.function_cx.current_stable_closure_path_prefix {
            Some(prefix) => format!("{prefix}.$lambda{ordinal}"),
            None => format!("$lambda{ordinal}"),
        };
        self.function_cx
            .stable_closure_paths
            .insert(closure_id, path.clone());
        path
    }

    pub(in crate::llvm::codegen) fn stable_closure_key_for_hir_closure(
        &mut self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
    ) -> Result<StableClosureKey, LlvmEmitError> {
        let owner_key = self.current_stable_owner_key(at, "stable closure owner key")?;
        let lexical_path = self.next_stable_child_closure_path(closure.id);
        Ok(StableClosureKey::new(&owner_key, lexical_path))
    }

    pub(in crate::llvm::codegen) fn stable_closure_key_for_materialized_callable(
        &self,
        callable_fqn: &str,
        at: crate::span::Span,
    ) -> Result<StableClosureKey, LlvmEmitError> {
        let Some((_, mir_fun)) = self.materialized_mir_callable(callable_fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "materialized MIR closure stable key",
                at: at.into(),
            });
        };
        if !mir_fun.name.starts_with("$lambda") {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "materialized MIR closure stable key",
                at: at.into(),
            });
        }
        let mut owner_callable_fqn = callable_fqn;
        while let Some((parent, _)) = owner_callable_fqn.rsplit_once(".$lambda") {
            owner_callable_fqn = parent;
        }
        let owner_fun = self.hir_fun_for_callable_fqn(owner_callable_fqn).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "materialized MIR closure owner",
                at: at.into(),
            },
        )?;
        let lexical_path = hir::stable_closure_lexical_path_in_fun(owner_fun, mir_fun.span)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "无法为 materialized MIR closure `{callable_fqn}` 从 HIR body 恢复稳定 lexical path"
                ),
            })?;

        if let Some(pass_view) = self.materialized_pass_view()
            && let Some(owner) = pass_view.owner_of_callable(owner_callable_fqn)
            && (!owner.type_args.is_empty() || !owner.eff_args.is_empty())
        {
            let stable_instance = pass_view.materialized().stable_instance_key(owner).ok_or_else(|| {
                LlvmEmitError::Frontend {
                    message: format!(
                        "无法为 materialized MIR closure `{callable_fqn}` 找到 authoritative stable instance key"
                    ),
                }
            })?;
            return Ok(StableClosureKey::new(stable_instance, lexical_path));
        }

        let owner_key = self.stable_def_key_for_current_cone(
            StableDefNamespace::Fun,
            &owner_fun.fqn,
            "top_level_fun",
        );
        Ok(StableClosureKey::new(&owner_key, lexical_path))
    }

    pub(in crate::llvm::codegen) fn direct_hir_closure_callable_fqn(
        &self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
    ) -> Result<String, LlvmEmitError> {
        let owner_fqn = self.function_cx.current_callable_fqn.as_deref().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "closure callable owner fqn",
                at: at.into(),
            },
        )?;
        Ok(format!("{owner_fqn}.$lambda{}", closure.id.as_u32()))
    }
}
