//! MainCodegen lifecycle (new / fresh_child_codegen) and stable identity: stable def keys, exported ABI symbol reservation, closure stable keys.

#![allow(dead_code)]

use super::*;
impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn new(shared: &'a CompilationUnitCodegenCx<'a, 'ctx>) -> Self {
        Self {
            shared,
            active_lir_program: None,
            current_source_id: shared.entry_source_id,
            function_cx: FunctionBodyCodegenCx::default(),
            effect_cx: EffectLoweringCodegenCx::default(),
        }
    }

    /// 统一 nested/wrapper codegen 的构造路径，避免再次手写整套编译单元输入拼装。
    pub(in crate::llvm::codegen) fn fresh_child_codegen(&self) -> Self {
        let mut child = Self::new(self.shared);
        child.active_lir_program = self.active_lir_program;
        child
    }

    pub(in crate::llvm::codegen) fn source_cone_info_for_path(
        &self,
        path: &Path,
    ) -> Option<&SourceConeInfo> {
        self.shared.source_cones.get(path)
    }

    pub(in crate::llvm::codegen) fn current_source_cone_info(&self) -> Option<&SourceConeInfo> {
        let source = self.source_map.source(self.current_source_id)?;
        self.source_cone_info_for_path(source.path())
    }

    pub(in crate::llvm::codegen) fn stable_cone_key_for_source_path(
        &self,
        path: &Path,
    ) -> &StableConeKey {
        self.source_cone_info_for_path(path)
            .map(|info| &info.stable_key)
            .unwrap_or(self.stable_cone_key)
    }

    pub(in crate::llvm::codegen) fn stable_cone_key_for_current_source(&self) -> &StableConeKey {
        self.current_source_cone_info()
            .map(|info| &info.stable_key)
            .unwrap_or(self.stable_cone_key)
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_current_cone(
        &self,
        namespace: StableDefNamespace,
        owner_path: &str,
        declaration_kind: &str,
    ) -> StableDefKey {
        StableDefKey::new(
            self.stable_cone_key_for_current_source().clone(),
            namespace,
            owner_path,
            declaration_kind,
            None,
        )
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_source_path(
        &self,
        source_path: &Path,
        namespace: StableDefNamespace,
        owner_path: &str,
        declaration_kind: &str,
    ) -> StableDefKey {
        StableDefKey::new(
            self.stable_cone_key_for_source_path(source_path).clone(),
            namespace,
            owner_path,
            declaration_kind,
            None,
        )
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_lir_global_root(
        &self,
        root: &LirGlobalRootFacts,
        namespace: StableDefNamespace,
        declaration_kind: &str,
    ) -> StableDefKey {
        if let Some(source_path) = root.source_path.as_deref() {
            return self.stable_def_key_for_source_path(
                Path::new(source_path),
                namespace,
                root.root.as_str(),
                declaration_kind,
            );
        }
        self.stable_def_key_for_current_cone(namespace, root.root.as_str(), declaration_kind)
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

    pub(in crate::llvm::codegen) fn stable_top_level_immutable_value_key_for_source_path(
        &self,
        source_path: &Path,
        value_fqn: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_source_path(
            source_path,
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

    pub(in crate::llvm::codegen) fn stable_top_level_init_key_for_source_path(
        &self,
        source_path: &Path,
        value_fqn: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_source_path(
            source_path,
            StableDefNamespace::TopLevelInit,
            value_fqn,
            "top_level_init",
        )
    }

    pub(in crate::llvm::codegen) fn stable_top_level_var_key(&self, var_fqn: &str) -> StableDefKey {
        self.stable_def_key_for_current_cone(StableDefNamespace::Value, var_fqn, "top_level_var")
    }

    pub(in crate::llvm::codegen) fn stable_top_level_var_key_for_source_path(
        &self,
        source_path: &Path,
        var_fqn: &str,
    ) -> StableDefKey {
        self.stable_def_key_for_source_path(
            source_path,
            StableDefNamespace::Value,
            var_fqn,
            "top_level_var",
        )
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
        let ty = self
            .try_mono_type_id(ty)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!("{context} 缺少 codegen type store"),
            })?;
        canonical_type_text(self.types, ty.inner(), self.stable_type_param_resolver()).map_err(
            |err| LlvmEmitError::Frontend {
                message: format!("{context} 无法构造 stable canonical type key: {err}"),
            },
        )
    }

    pub(in crate::llvm::codegen) fn stable_rtti_type_id_for_codegen(
        &self,
        ty: TypeId,
        context: &str,
    ) -> Result<u64, LlvmEmitError> {
        let ty = self
            .try_mono_type_id(ty)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!("{context} 缺少 codegen type store"),
            })?;
        stable_rtti_type_id_for_type(self.types, ty.inner(), self.stable_type_param_resolver())
            .map_err(|err| LlvmEmitError::Frontend {
                message: format!("{context} 无法构造 stable RTTI type id: {err}"),
            })
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_callable_signature(
        &self,
        owner_path: &str,
        declaration_kind: &str,
        callable_ty: TypeId,
        types: &TypeStore,
    ) -> Result<StableDefKey, LlvmEmitError> {
        self.stable_def_key_for_callable_signature_in_cone(
            self.stable_cone_key_for_current_source(),
            owner_path,
            declaration_kind,
            callable_ty,
            types,
        )
    }

    pub(in crate::llvm::codegen) fn stable_def_key_for_callable_signature_in_cone(
        &self,
        stable_cone_key: &StableConeKey,
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
            stable_cone_key.clone(),
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

    pub(in crate::llvm::codegen) fn lir_callable_symbol_facts(
        &self,
        callable_id: LirCallableId,
    ) -> Option<&'a scoopc_lir_facts::LirCallableSymbolFacts> {
        self.active_lir_program()?
            .physical_layout()
            .callable_symbols
            .get(&callable_id)
    }

    pub(in crate::llvm::codegen) fn abi_symbol_for_lir_callable_ref(
        &self,
        callable: scoopc_lir_facts::LirCallableRef,
    ) -> Option<&'a scoopc_lir_facts::LirAbiSymbolFact> {
        self.active_lir_program()?
            .physical_layout()
            .abi_symbols
            .values()
            .find(|symbol| symbol.callable == Some(callable))
    }

    pub(in crate::llvm::codegen) fn exported_abi_symbol_for_lir_callable_ref(
        &self,
        callable: scoopc_lir_facts::LirCallableRef,
    ) -> Result<String, LlvmEmitError> {
        let program = self.expect_active_lir_program("exported_abi_symbol_for_lir_callable_ref");
        let root = program.root_for_callable_ref(callable);
        let owner_label = root.unwrap_or("<external-lir-callable>");
        if root == Some("main") {
            return Ok("main".to_string());
        }
        if let scoopc_lir_facts::LirCallableRef::Local(id) = callable
            && let Some(symbol_facts) = self.lir_callable_symbol_facts(id)
        {
            let symbol = symbol_facts
                .exported_symbol
                .clone()
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!(
                        "LIR callable symbol facts for `{owner_label}` are missing exported ABI symbol"
                    ),
                })?;
            self.reserve_exported_abi_symbol(
                &symbol,
                &self.lir_callable_ref_stable_key(scoopc_lir_facts::LirCallableRef::Local(
                    symbol_facts.callable,
                ))?,
                format!("LIR callable `{owner_label}` via callable symbol facts"),
            )?;
            return Ok(symbol);
        }
        if let Some(abi_symbol) = self.abi_symbol_for_lir_callable_ref(callable) {
            if let Some(callable) = abi_symbol.callable.as_ref() {
                self.reserve_exported_abi_symbol(
                    &abi_symbol.symbol,
                    &self.lir_callable_ref_stable_key(*callable)?,
                    format!("LIR declaration `{owner_label}` via ABI symbol facts"),
                )?;
            }
            return Ok(abi_symbol.symbol.clone());
        }
        Err(LlvmEmitError::Frontend {
            message: format!(
                "LIR callable `{owner_label}` is missing a published target-bound ABI symbol fact"
            ),
        })
    }

    pub(in crate::llvm::codegen) fn exported_abi_symbol_for_lir_callable_id(
        &self,
        callable_id: LirCallableId,
    ) -> Result<String, LlvmEmitError> {
        self.exported_abi_symbol_for_lir_callable_ref(scoopc_lir_facts::LirCallableRef::Local(
            callable_id,
        ))
    }

    pub(in crate::llvm::codegen) fn enter_root_callable_identity(
        &mut self,
        callable_id: Option<LirCallableId>,
        stable_owner_key: StableDefKey,
    ) {
        self.function_cx.current_lir_callable_id = callable_id;
        self.function_cx.current_stable_owner_key = Some(stable_owner_key);
        self.function_cx.current_stable_closure_path_prefix = None;
        self.function_cx.next_stable_child_closure_index = 0;
        self.function_cx.stable_closure_paths.clear();
    }

    pub(in crate::llvm::codegen) fn enter_nested_closure_identity(
        &mut self,
        callable_id: Option<LirCallableId>,
        stable_owner_key: StableDefKey,
        stable_closure_path: &str,
    ) {
        self.function_cx.current_lir_callable_id = callable_id;
        self.function_cx.current_stable_owner_key = Some(stable_owner_key);
        self.function_cx.current_stable_closure_path_prefix = Some(stable_closure_path.to_string());
        self.function_cx.next_stable_child_closure_index = 0;
        self.function_cx.stable_closure_paths.clear();
    }

    fn lir_callable_ref_stable_key(
        &self,
        callable: scoopc_lir_facts::LirCallableRef,
    ) -> Result<scoopc_ids::CanonicalTextKey, LlvmEmitError> {
        let canonical = match callable {
            scoopc_lir_facts::LirCallableRef::Local(id) => self
                .active_lir_program()
                .and_then(|program| program.callable_by_id(id))
                .and_then(|callable| callable.published_callable_facts())
                .map(|facts| facts.body_version.key.owner_canonical_text().to_string())
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!(
                        "LIR callable ref {id:?} is missing callable facts for ABI symbol reservation"
                    ),
                })?,
            scoopc_lir_facts::LirCallableRef::ExternalHash(hash) => {
                format!("lir_callable_hash#h{}", hash.to_hex())
            }
        };
        Ok(scoopc_ids::CanonicalTextKey::new(canonical))
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

    pub(in crate::llvm::codegen) fn stable_closure_key_for_lir_callable_id(
        &self,
        callable_id: LirCallableId,
        _at: crate::span::Span,
    ) -> Result<StableClosureKey, LlvmEmitError> {
        let program =
            self.published_late_lowered_program()
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: "LIR closure identity lookup requires a published LIR program"
                        .to_string(),
                })?;
        let callable = program.callable_by_id(callable_id).unwrap_or_else(|| {
            panic!("stable_closure_key_for_lir_callable_id: LIR source contract accepted missing closure callable")
        });
        let source_fun = callable.source_callable().unwrap_or_else(|| {
            panic!("stable_closure_key_for_lir_callable_id: LIR source contract accepted callable without source contract")
        });
        if !source_fun.name.starts_with("$lambda") {
            panic!("stable_closure_key_for_lir_callable_id: callable is not a closure body")
        }
        let identity = self
            .expect_active_lir_program("stable_closure_key_for_lir_callable_id")
            .physical_layout()
            .closure_identities
            .get(&callable_id)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LIR closure identity facts 缺少 closure `{}` 的 owner/lexical path",
                    source_fun.fqn,
                ),
            })?;
        let owner_key = program
            .callable_by_id(identity.owner_callable)
            .and_then(|callable| callable.lir_callable_key())
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LIR closure identity facts 缺少 owner `{}` 的 stable callable key",
                    identity.owner_root_fqn
                ),
            })?;
        Ok(StableClosureKey::new(
            owner_key,
            identity.lexical_path.as_str(),
        ))
    }

    pub(in crate::llvm::codegen) fn direct_hir_closure_callable_fqn(
        &self,
        _at: crate::span::Span,
        closure: &hir::ClosureExpr,
    ) -> Result<String, LlvmEmitError> {
        let owner = self.current_callable_diagnostic_label();
        if owner == "<unknown>" {
            panic!(
                "direct_hir_closure_callable_fqn: closure codegen requires current callable owner"
            )
        }
        Ok(format!("{owner}.$lambda{}", closure.id.as_u32()))
    }
}
