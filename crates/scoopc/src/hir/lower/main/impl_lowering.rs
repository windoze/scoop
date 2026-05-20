//! HirLowering core impl: AST→HIR lowering for declarations, expressions, statements, types and patterns.

#![allow(dead_code)]

use super::*;

impl<'a> HirLowering<'a> {
    pub(in crate::hir::lower) const PROPERTY_META_FQN: &'static str = "scoop.core.PropertyMeta";
    pub(in crate::hir::lower) const MUTABLE_ARRAY_NEW_FQN: &'static str =
        "scoop.core.mutableArrayNew";
    pub(in crate::hir::lower) const MUTABLE_ARRAY_PUSH_FQN: &'static str = "scoop.core.push";
    pub(in crate::hir::lower) const MUTABLE_ARRAY_FREEZE_FQN: &'static str = "scoop.core.freeze";
    pub(in crate::hir::lower) const INT_PROGRESSION_FQN: &'static str = "scoop.core.IntProgression";
    pub(in crate::hir::lower) const LONG_PROGRESSION_FQN: &'static str =
        "scoop.core.LongProgression";
    pub(in crate::hir::lower) const UINT_PROGRESSION_FQN: &'static str =
        "scoop.core.UIntProgression";
    pub(in crate::hir::lower) const ULONG_PROGRESSION_FQN: &'static str =
        "scoop.core.ULongProgression";
    pub(in crate::hir::lower) const RANGE_TO_FQN: &'static str = "scoop.core.rangeTo";
    pub(in crate::hir::lower) const DOWN_TO_FQN: &'static str = "scoop.core.downTo";
    pub(in crate::hir::lower) const UNTIL_FQN: &'static str = "scoop.core.until";
    pub(in crate::hir::lower) const RANGE_DEFAULT_STEP_FQN: &'static str =
        "scoop.core.__scoop_range_default_step";
    pub(in crate::hir::lower) const STRING_BUILDER_FQN: &'static str =
        "scoop.lang.string.StringBuilder";
    pub(in crate::hir::lower) const STRING_BUILDER_ADD_FQN: &'static str =
        "scoop.lang.string.StringBuilder.add";
    pub(in crate::hir::lower) const STRING_BUILDER_TO_STRING_FQN: &'static str =
        "scoop.lang.string.StringBuilder.toString";
    pub(in crate::hir::lower) const TO_STRING_INTERFACE_METHOD_FQN: &'static str =
        "scoop.core.ToString.toString";
    pub(in crate::hir::lower) const SYNC_MUTEX_TYPE_FQN: &'static str = "scoop.sync.Mutex";
    pub(in crate::hir::lower) const SYNC_MUTEX_CREATE_FQN: &'static str = "scoop.sync.mutexCreate";
    pub(in crate::hir::lower) const SYNC_MUTEX_LOCK_FQN: &'static str = "scoop.sync.lock";
    pub(in crate::hir::lower) const SYNC_MUTEX_UNLOCK_FQN: &'static str = "scoop.sync.unlock";
    pub(in crate::hir::lower) const RAISE_RAISE_FQN: &'static str = "scoop.core.Raise.raise";
    pub(in crate::hir::lower) const RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN: &'static str =
        "scoop.core.RuntimeError.NullAssertionFailed";

    pub(crate) fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        types: &'a mut TypeStore,
        setup: HirLoweringSetup<'a>,
    ) -> Self {
        let HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties,
            compilation_unit,
            default_arg_structs,
            computed_property_getters,
            computed_property_setters,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
        } = setup;
        Self {
            source,
            file,
            index,
            typecheck_types,
            type_kinds,
            delegated_properties,
            compilation_unit,
            default_arg_funs: HashMap::new(),
            default_arg_structs,
            computed_property_getters,
            computed_property_setters,
            ctor_call_sites: HashMap::new(),
            dispatch_call_sites: HashMap::new(),
            effect_op_call_sites: HashMap::new(),
            handle_payload_tuple_tys: HashMap::new(),
            with_update_contracts: HashMap::new(),
            assign_place_contracts: HashMap::new(),
            top_level_vars: HashMap::new(),
            extern_globals: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            when_pat_binding_tys: HashMap::new(),
            symbols: SymbolInterner::default(),
            local_mutability: HashMap::new(),
            local_decl_tys: HashMap::new(),
            next_closure: 0,
            lambda_this_decl_span: None,
            next_synthetic_local: 0,
            next_synthetic_call_site: 0,
            types,
            builtins,
            type_param_scopes: Vec::new(),
            effect_row_param_scopes: Vec::new(),
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            local_decl_span_overrides: Vec::new(),
            stage_error: None,
        }
    }

    pub(crate) fn record_stage_error(
        &mut self,
        span: Span,
        reason: impl Into<String>,
        owner: impl Into<String>,
    ) {
        if self.stage_error.is_none() {
            self.stage_error = Some(HirStageError::new(
                self.source.path().to_path_buf(),
                span,
                reason,
                owner,
            ));
        }
    }

    pub(crate) fn take_stage_error(&mut self) -> Option<HirStageError> {
        self.stage_error.take()
    }

    pub(crate) fn intern_local_symbol(&mut self, decl_span: Span, mutable: bool) -> SymbolId {
        let decl_span = self.remap_local_decl_span(decl_span);
        let id = self.symbols.intern_local(self.source.path(), decl_span);
        match self.local_mutability.get(&id).copied() {
            // 同一 decl_span 不应出现冲突的 mutability，但为了降低与 resolver 交互时的脆弱性：
            // - 若任一方认为它是 `var`，则提升为 `var`；
            // - 否则保持 `val`。
            Some(prev) => {
                let _ = self.local_mutability.insert(id, prev || mutable);
            }
            None => {
                self.local_mutability.insert(id, mutable);
            }
        }
        id
    }

    pub(crate) fn remap_local_decl_span(&self, decl_span: Span) -> Span {
        for scope in self.local_decl_span_overrides.iter().rev() {
            if let Some(remapped) = scope.get(&decl_span) {
                return *remapped;
            }
        }
        decl_span
    }

    pub(crate) fn with_local_decl_span_overrides<T>(
        &mut self,
        overrides: HashMap<Span, Span>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.local_decl_span_overrides.push(overrides);
        let result = f(self);
        let _ = self.local_decl_span_overrides.pop();
        result
    }

    pub(crate) fn record_when_pat_binding_ty(&mut self, decl_span: Span, ty: TypeId) {
        let site = crate::hir::WhenPatBindingSite {
            source_path: self.source.path().to_path_buf(),
            decl_span,
        };
        self.when_pat_binding_tys.insert(site, ty);
    }

    pub(crate) fn intern_effect_row_param_marker(
        &mut self,
        name: String,
        decl_span: Span,
    ) -> TypeId {
        self.types.intern(TypeKind::Param(TypeParamType {
            name,
            decl_file: std::path::PathBuf::from(EFFECT_ROW_PARAM_DECL_FILE),
            decl_span,
        }))
    }

    pub(crate) fn push_effect_row_param_placeholder(&mut self, name: String, decl_span: Span) {
        let mut scope = HashMap::new();
        let marker = self.intern_effect_row_param_marker(name.clone(), decl_span);
        scope.insert(name, EffectRowParamBinding::Placeholder(marker));
        self.effect_row_param_scopes.push(scope);
    }

    pub(crate) fn push_effect_row_param_binding(&mut self, name: String, row: EffectRow) {
        let mut scope = HashMap::new();
        scope.insert(name, EffectRowParamBinding::Concrete(row));
        self.effect_row_param_scopes.push(scope);
    }

    pub(crate) fn effect_row_param_is_bound(&self, name: &str) -> bool {
        self.effect_row_param_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    pub(crate) fn push_missing_fun_effect_placeholder(
        &mut self,
        eff_param: Option<&ast::EffectRowParam>,
    ) -> bool {
        let Some(eff_param) = eff_param else {
            return false;
        };
        let name = eff_param.name.text(self.source).to_string();
        if self.effect_row_param_is_bound(&name) {
            return false;
        }
        self.push_effect_row_param_placeholder(name, eff_param.name.span);
        true
    }

    pub(crate) fn pop_effect_row_param_binding(&mut self) {
        let _ = self.effect_row_param_scopes.pop();
    }

    pub(crate) fn call_site(&self, span: Span) -> CallSite {
        CallSite::new(self.source.path().to_path_buf(), span)
    }

    pub(crate) fn fresh_synthetic_local(
        &mut self,
        anchor: Span,
        prefix: &str,
        mutable: bool,
    ) -> (Span, SymbolId, String) {
        let index = self.next_synthetic_local;
        self.next_synthetic_local = self.next_synthetic_local.saturating_add(1);

        // 刻意把合成 span 放到文件末尾之后，避免与真实源码 decl span / call-site 冲突。
        let base = self
            .source
            .text()
            .len()
            .saturating_add(index.saturating_mul(2));
        let span = Span::new(base, base.saturating_add(1));
        let id = self.intern_local_symbol(span, mutable);
        let name = format!("{prefix}_{index}");
        let _ = anchor; // 目前仅保留参数，便于后续若需改成“锚定到原语句附近”时不改调用点。
        (span, id, name)
    }

    pub(crate) fn fresh_synthetic_call_site_span(&mut self, anchor: Span) -> Span {
        let index = self.next_synthetic_call_site;
        self.next_synthetic_call_site = self.next_synthetic_call_site.saturating_add(1);

        // helper call-site 需要与既有 synthetic local span 稳定隔离，避免仅因 call-site identity
        // 修复而重排大量临时 local span / snapshot。
        let base = self
            .source
            .text()
            .len()
            .saturating_add(1 << 20)
            .saturating_add(index.saturating_mul(2));
        let _ = anchor;
        Span::new(base, base.saturating_add(1))
    }

    pub(crate) fn with_foreign_ast_context<T>(
        &mut self,
        source: &'a SourceFile,
        file: &'a ast::File,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous_source = self.source;
        let previous_file = self.file;
        self.source = source;
        self.file = file;
        let result = f(self);
        self.source = previous_source;
        self.file = previous_file;
        result
    }

    pub(crate) fn with_lambda_this_decl_span<T>(
        &mut self,
        lambda_this_decl_span: Option<Span>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous = self.lambda_this_decl_span;
        self.lambda_this_decl_span = lambda_this_decl_span;
        let result = f(self);
        self.lambda_this_decl_span = previous;
        result
    }

    pub(crate) fn lower_file(&mut self) -> File {
        let pkg_prefix = package_prefix(self.source, self.file.package.as_ref());
        self.default_arg_funs = self.collect_default_arg_funs(&pkg_prefix);
        let mut decls = Vec::new();
        let mut items = Vec::with_capacity(self.file.items.len());

        for item in &self.file.items {
            self.lower_item_into(&pkg_prefix, item, &mut items, &mut decls);
        }

        File { decls, items }
    }

    /// 扫描当前源文件内的顶层 `fun` 声明，收集“默认参数”信息（供 call-site 默认参数补齐使用）。
    ///
    /// 注意：
    /// - 这里只服务于早期单文件 codegen，因此只索引“当前文件内”的顶层函数；
    /// - 不尝试处理 overload set（同名重载）与泛型函数（它们需要 typecheck 的最终决议信息）。
    pub(crate) fn collect_default_arg_funs(
        &mut self,
        pkg_prefix: &str,
    ) -> HashMap<String, DefaultArgFunInfo> {
        let mut out: HashMap<String, DefaultArgFunInfo> = HashMap::new();

        for item in &self.file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };

            // 当前阶段：默认参数补齐仅针对“非泛型 + 非 receiver”的顶层函数。
            if fun.receiver.is_some() || !fun.type_params.is_empty() {
                continue;
            }
            // 当前阶段：不处理 vararg（其“缺省语义”是空数组/0 个元素，而非 param default_value）。
            if fun.params.iter().any(|p| p.is_vararg) {
                continue;
            }

            let name = fun.name.text(self.source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 过滤 overload set：同一个 fqn 出现多次时不做索引（避免与 typecheck 的 overload 决议冲突）。
            if out.contains_key(&fqn) {
                let _ = out.remove(&fqn);
                continue;
            }

            let mut params: Vec<DefaultArgParamInfo> = Vec::with_capacity(fun.params.len());
            for p in &fun.params {
                let name = p.name.text(self.source).to_string();
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name,
                    is_vararg: p.is_vararg,
                    ty_ref: p.ty.clone(),
                    default_value: p.default_value.clone(),
                });
            }

            if !params.iter().any(|p| p.default_value.is_some()) {
                continue;
            }

            let required = params.iter().filter(|p| p.default_value.is_none()).count();
            out.insert(fqn, DefaultArgFunInfo { required, params });
        }

        out
    }

    pub(crate) fn lower_item_into(
        &mut self,
        pkg_prefix: &str,
        item: &ast::Item,
        out: &mut Vec<Item>,
        decls: &mut Vec<Decl>,
    ) {
        match item {
            ast::Item::Fun(fun) => out.push(Item::Fun(self.lower_fun_decl(pkg_prefix, fun))),
            ast::Item::Val(v) if matches!(v.binding, ast::ValBinding::Pattern(_)) => {
                self.lower_top_level_pattern_val_items(pkg_prefix, v, out);
            }
            ast::Item::Val(v) => out.push(Item::Val(self.lower_val_decl(
                pkg_prefix,
                v,
                ValScope::TopLevel,
            ))),
            ast::Item::TypeAlias(ta) => {
                decls.push(Decl::TypeAlias(self.lower_typealias_decl(pkg_prefix, ta)));
            }
            ast::Item::Type(ty) => {
                decls.push(Decl::Nominal(self.lower_nominal_decl(pkg_prefix, ty)));
            }
            ast::Item::Object(obj) => {
                if let Some(decl) = self.lower_object_decl(pkg_prefix, obj) {
                    decls.push(Decl::Object(decl));
                }
            }
            ast::Item::ExtensionProperty(p) => {
                decls.push(Decl::ExtensionProperty(
                    self.lower_extension_property_decl(pkg_prefix, p),
                ));
                if let Some(getter) = self.lower_extension_property(pkg_prefix, p) {
                    out.push(getter);
                }
            }
        }
    }

    /// Collect member functions and value-type computed getters into the callable side table.
    pub(crate) fn collect_member_funs(&mut self, pkg_prefix: &str) -> Vec<FunDecl> {
        let mut out: Vec<FunDecl> = Vec::new();

        for item in &self.file.items {
            match item {
                ast::Item::Type(ty) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, ty, pkg_prefix, &mut out);
                }
                ast::Item::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, pkg_prefix, &mut out);
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_) => {}
            }
        }

        out
    }

    pub(crate) fn collect_member_funs_in_type_decl(
        &mut self,
        pkg_prefix: &str,
        decl: &ast::TypeDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let local_name = decl.name.text(self.source);
        let owner_fqn = join_prefix(prefix, local_name);

        let Some(body) = &decl.body else {
            return;
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Property(prop) => {
                    if should_lower_computed_property_getter(prop) {
                        out.push(self.lower_value_property_getter_decl(
                            pkg_prefix,
                            &owner_fqn,
                            &decl.type_params,
                            decl.name.span,
                            prop,
                        ));
                    }
                    if should_lower_computed_property_setter(prop) {
                        out.push(self.lower_computed_property_setter_decl(
                            pkg_prefix,
                            &owner_fqn,
                            &decl.type_params,
                            decl.name.span,
                            prop,
                        ));
                    }
                }
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &decl.type_params,
                        decl.name.span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
    }

    pub(crate) fn collect_member_funs_in_object_decl(
        &mut self,
        pkg_prefix: &str,
        obj: &ast::ObjectDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let Some(name) = object_decl_name(self.source, obj) else {
            return;
        };
        let owner_fqn = join_prefix(prefix, &name);

        let this_decl_span = obj.name.as_ref().map(|n| n.span).unwrap_or(obj.span);

        let Some(body) = &obj.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &[],
                        this_decl_span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(nested) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
    }

    pub(crate) fn lower_fun_decl(&mut self, pkg_prefix: &str, fun: &ast::FunDecl) -> FunDecl {
        // 进入函数作用域：先把 type params lower 成 `TypeId`，保证签名与 body 内引用一致。
        self.push_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 扩展函数的 receiver（`fun T.f(...)`）在 resolver 中会把 `this` 解析为一个局部绑定，
        // 且 decl_span 取 receiver type 的 span（见 `resolve::scopes` 中 `ThisContext.decl_span`）。
        // 为了让 codegen 能把 `this` 当作一个普通局部参数处理，这里把 receiver 显式降为第 0 个参数。
        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        // receiver 已作为显式参数降入 `params`，因此 HIR 的 function type 不再单独保留 receiver 位。
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params();

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// T0112: Synthesize an extension property's getter as a top-level function.
    ///
    /// The getter function has:
    /// - FQN = property FQN (e.g., `pkg.lastIndex`)
    /// - params = `[this: ReceiverType]`
    /// - return type = declared property type
    /// - body = getter body
    pub(crate) fn lower_extension_property(
        &mut self,
        pkg_prefix: &str,
        prop: &ast::ExtensionPropertyDecl,
    ) -> Option<Item> {
        let Some(getter) = &prop.getter else {
            return None;
        };

        self.push_type_params(&prop.type_params);

        let name = self.source.slice(prop.name.span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = self.lower_type_ref(&prop.receiver);

        // Receiver as first parameter (same as extension functions).
        let receiver_span = prop.receiver.span();
        let receiver_id = self.intern_local_symbol(receiver_span, false);
        let params = vec![Param {
            span: receiver_span,
            id: receiver_id,
            name: "this".to_string(),
            ty: receiver_ty,
        }];

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let effects = EffectRow::pure();
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);

        // Lower getter body.
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                // `get() = expr` → synthesize a block with the expression as tail stmt.
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.pop_type_params();

        Some(Item::Fun(FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }))
    }

    /// 降低一个 class/object 的 member `fun` 为”顶层函数形态”（显式 `this` 参数）。
    ///
    /// 约定：
    /// - `this_decl_span` 必须与 resolver 为 `this` 写回的 `ResolvedValueRef::Local { decl_span }` 对齐：
    ///   - class member：`decl.name.span`
    ///   - object member：`obj.name.span`（匿名 companion 用 `obj.span`）
    pub(crate) fn lower_member_fun_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // owner type params 在 member 方法体内可见（例如 `class Box<T> { fun get(): T }`）。
        self.push_type_params(owner_type_params);
        self.push_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params(); // fun type params
        self.pop_type_params(); // owner type params

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 将值类型（struct/enum）的 getter-only computed property 降低为“顶层函数形态”。
    ///
    /// 约定：
    /// - FQN 直接复用属性 FQN（例如 `pkg.Point.doubled`）；
    /// - 第 0 个参数为显式 `this`；
    /// - body 直接来自 accessor getter body。
    pub(crate) fn lower_value_property_getter_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        prop: &ast::PropertyDecl,
    ) -> FunDecl {
        let getter = prop.getter.as_ref().expect(
            "computed property getter collection only calls this helper for getter-only properties",
        );

        self.push_type_params(owner_type_params);

        let name = prop.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);
        let params = vec![Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        }];
        let previous_this_ty = self.push_synthetic_local_decl_ty(this_decl_span, this_ty);

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            EffectRow::pure(),
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.restore_synthetic_local_decl_ty(this_decl_span, previous_this_ty);

        self.pop_type_params();

        FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 将无 backing field 的 computed property setter 降低为 HIR 函数。
    ///
    /// 约定：
    /// - FQN 使用内部 setter 符号（例如 `pkg.Box.value$set`），避免与 getter/property FQN 冲突；
    /// - 第 0 个参数为显式 `this`，第 1 个参数为 setter 的 `value`；
    /// - body 直接来自 accessor setter body。
    pub(crate) fn lower_computed_property_setter_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        prop: &ast::PropertyDecl,
    ) -> FunDecl {
        let setter = prop.setter.as_ref().expect(
            "computed property setter collection only calls this helper for setter properties",
        );

        self.push_type_params(owner_type_params);

        let property_name = prop.name.text(self.source).to_string();
        let property_fqn = format!("{owner_fqn}.{property_name}");
        let fqn = computed_property_setter_fqn(&property_fqn);
        let name = format!("{property_name}$set");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);

        let value_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let (value_span, value_name) = setter
            .param
            .as_ref()
            .map(|param| (param.span, param.text(self.source).to_string()))
            .unwrap_or((prop.name.span, "value".to_string()));
        let value_id = self.intern_local_symbol(value_span, false);

        let params = vec![
            Param {
                span: this_decl_span,
                id: this_id,
                name: "this".to_string(),
                ty: this_ty,
            },
            Param {
                span: value_span,
                id: value_id,
                name: value_name,
                ty: value_ty,
            },
        ];
        let previous_this_ty = self.push_synthetic_local_decl_ty(this_decl_span, this_ty);
        let previous_value_ty = self.push_synthetic_local_decl_ty(value_span, value_ty);

        let return_ty = self.builtins.unit;
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            EffectRow::pure(),
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &setter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                Some(Block {
                    span: e.span,
                    ty: return_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: return_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.restore_synthetic_local_decl_ty(value_span, previous_value_ty);
        self.restore_synthetic_local_decl_ty(this_decl_span, previous_this_ty);

        self.pop_type_params();

        FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 在“已绑定 type params”的语境下降低一个函数声明。
    ///
    /// 用途：
    /// - 单态化（monomorphization）生成具体实例：把 `T` 等 type param 直接映射到具体 `TypeId`
    ///   后再构造 HIR（避免再次生成 `TypeKind::Param`）。
    pub(crate) fn lower_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());
        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        let out = FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        };
        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        out
    }

    /// T0126: 在"已绑定 owner type params"的语境下降低成员方法。
    ///
    /// 与 `lower_member_fun_decl` 的区别：owner type params 已经通过 `push_type_param_bindings`
    /// 预绑定到具体类型（由调用方在调用前完成），因此 `this` 参数和方法体中的 owner type params
    /// 均会直接解析为具体 TypeId。
    ///
    /// `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于
    /// 构造 `this` 参数的精确 nominal 类型（codegen 阶段需要从 `this.hir_ty` 提取 type args
    /// 来查找 class field 布局）。
    pub(crate) fn lower_member_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        this_decl_span: Span,
        this_concrete_args: &[TypeId],
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // 方法自身的 type params（如果有的话）仍然需要 push；
        // owner 的 type params 已由调用方在 push_type_param_bindings 中绑定。
        let fun_type_params_pushed = self.push_missing_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_concrete_args.to_vec(), None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => Some(self.lower_block(pkg_prefix, b)),
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        if fun_type_params_pushed {
            self.pop_type_params(); // fun type params
        }

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 将值类型 computed property getter 在“已绑定 owner type params”的语境下降低为 HIR。
    pub(crate) fn lower_value_property_getter_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        this_decl_span: Span,
        this_concrete_args: &[TypeId],
        prop: &ast::PropertyDecl,
    ) -> FunDecl {
        let getter = prop.getter.as_ref().expect(
            "computed property getter collection only calls this helper for getter-only properties",
        );

        let name = prop.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_concrete_args.to_vec(), None);
        let params = vec![Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        }];
        let previous_this_ty = self.push_synthetic_local_decl_ty(this_decl_span, this_ty);

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            EffectRow::pure(),
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.restore_synthetic_local_decl_ty(this_decl_span, previous_this_ty);

        FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            ty,
            params,
            return_ty,
            body,
        }
    }

    pub(crate) fn call_top_level_fun(
        &mut self,
        span: Span,
        fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Expr {
        let call_span = self.fresh_synthetic_call_site_span(span);
        let fqn = fqn.to_string();
        let callee = Expr {
            span: call_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span: call_span,
            ty: ret_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: args.into_iter().map(CallArg::Positional).collect(),
            },
        }
    }

    pub(crate) fn call_top_level_fun_with_synthetic_binding(
        &mut self,
        span: Span,
        fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
        intrinsic_entry_name: Option<&str>,
    ) -> Expr {
        let expr = self.call_top_level_fun(span, fqn, args, ret_ty);
        self.record_synthetic_top_level_fun_call_binding(expr.span, fqn, intrinsic_entry_name);
        expr
    }

    /// Lower a compiler-generated member call through the same canonical HIR contract as source
    /// member calls: `receiver.method(args...)` becomes `Owner.method(receiver, args...)`, with
    /// interface/virtual dispatch recorded in the source-aware dispatch side table.
    pub(crate) fn lower_synthetic_member_call(
        &mut self,
        span: Span,
        receiver: Expr,
        method_fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Expr {
        let receiver_ty = receiver.ty;
        self.lower_synthetic_member_call_with_receiver_ty(
            span,
            receiver,
            receiver_ty,
            method_fqn,
            args,
            ret_ty,
        )
    }

    /// Variant of `lower_synthetic_member_call` for synthetic receiver expressions whose HIR type is
    /// less precise than the statically selected dispatch receiver type.
    pub(crate) fn lower_synthetic_member_call_with_receiver_ty(
        &mut self,
        span: Span,
        receiver: Expr,
        receiver_ty: TypeId,
        method_fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Expr {
        let mut target_fqn = method_fqn.to_string();
        if let Some((owner_fqn, member_name)) = method_fqn.rsplit_once('.') {
            let dispatch_kind = if matches!(
                self.type_kinds.get(owner_fqn),
                Some(ast::TypeKind::Interface)
            ) {
                Some(crate::hir::DispatchCallKind::Interface)
            } else if matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class))
                && owner_fqn != "scoop.core.String"
            {
                Some(crate::hir::DispatchCallKind::Virtual)
            } else {
                None
            };

            if let Some(dispatch_kind) = dispatch_kind {
                if self.devirtualize_dispatch_calls {
                    if let Some(devirtualized_target_fqn) =
                        crate::devirtualize::try_devirtualize_dispatch_target(
                            dispatch_kind,
                            owner_fqn,
                            member_name,
                            args.len(),
                            receiver_ty,
                            self.types,
                            crate::devirtualize::DispatchTargetFacts {
                                known_receiver_subclasses: self.known_receiver_subclasses,
                                class_vtables: self.class_vtables,
                                interfaces: self.interfaces,
                                class_itables: self.class_itables,
                            },
                        )
                    {
                        target_fqn = self.materialized_devirtualized_dispatch_target_fqn(
                            span,
                            &devirtualized_target_fqn,
                        );
                    } else {
                        self.dispatch_call_sites
                            .insert(self.dispatch_call_site(span, receiver_ty), dispatch_kind);
                    }
                } else {
                    self.dispatch_call_sites
                        .insert(self.dispatch_call_site(span, receiver_ty), dispatch_kind);
                }
            }
        }

        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(CallArg::Positional(receiver));
        call_args.extend(args.into_iter().map(CallArg::Positional));

        Expr {
            span,
            ty: ret_ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(target_fqn.clone()),
                        fqn: target_fqn,
                    }),
                }),
                args: call_args,
            },
        }
    }

    pub(crate) fn record_synthetic_top_level_fun_call_binding(
        &self,
        span: Span,
        fqn: &str,
        intrinsic_entry_name: Option<&str>,
    ) {
        let mut bindings = self.file.top_level_fun_call_bindings();
        if bindings.contains_key(&span) {
            return;
        }

        let (decl_file, decl_span, is_intrinsic) = self
            .index
            .by_fqn
            .get(fqn)
            .and_then(|syms| syms.fun.first())
            .map(|fun| {
                (
                    fun.symbol.decl_file.clone(),
                    fun.symbol.span,
                    fun.sig.builtin_flags.is_intrinsic,
                )
            })
            .unwrap_or_else(|| {
                (
                    self.source.path().to_path_buf(),
                    span,
                    intrinsic_entry_name.is_some(),
                )
            });

        bindings.insert(
            span,
            crate::ast::TopLevelFunCallBinding {
                fqn: fqn.to_string(),
                decl_file,
                decl_span,
                is_intrinsic,
                intrinsic_entry_name: intrinsic_entry_name.map(str::to_string),
                type_args: Vec::new(),
                eff_args: Vec::new(),
            },
        );
        self.file.replace_top_level_fun_call_bindings(bindings);
    }

    pub(crate) fn replace_synthetic_top_level_fun_call_binding(
        &self,
        span: Span,
        fqn: &str,
        intrinsic_entry_name: Option<&str>,
    ) {
        let mut bindings = self.file.top_level_fun_call_bindings();
        let (decl_file, decl_span, is_intrinsic) = self
            .index
            .by_fqn
            .get(fqn)
            .and_then(|syms| syms.fun.first())
            .map(|fun| {
                (
                    fun.symbol.decl_file.clone(),
                    fun.symbol.span,
                    fun.sig.builtin_flags.is_intrinsic,
                )
            })
            .unwrap_or_else(|| {
                (
                    self.source.path().to_path_buf(),
                    span,
                    intrinsic_entry_name.is_some(),
                )
            });

        bindings.insert(
            span,
            crate::ast::TopLevelFunCallBinding {
                fqn: fqn.to_string(),
                decl_file,
                decl_span,
                is_intrinsic,
                intrinsic_entry_name: intrinsic_entry_name.map(str::to_string),
                type_args: Vec::new(),
                eff_args: Vec::new(),
            },
        );
        self.file.replace_top_level_fun_call_bindings(bindings);
    }

    pub(crate) fn lower_val_decl(
        &mut self,
        pkg_prefix: &str,
        v: &ast::ValDecl,
        scope: ValScope,
    ) -> ValDecl {
        // T0124: lower the declared type first so we can pass it as expected type for struct literals.
        let declared_ty_early = v.ty.as_ref().map(|t| self.lower_type_ref(t));
        let typechecked_init_ty = v
            .init
            .as_ref()
            .and_then(|init| self.typechecked_expr_ty(init.span));

        // T1317c：数组字面量 `[...]` 的 lowering 依赖”期望的容器类型”（Array vs MutableArray）。
        // 这里从显式的类型注解（若存在）向 initializer 传播该 hint。
        let init_expected = ExpectedExpr {
            value_ty: declared_ty_early,
            array_lit_target: v
                .ty
                .as_ref()
                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
            array_lit_ty: declared_ty_early,
            struct_lit_ty: declared_ty_early,
        };
        let init = v
            .init
            .as_ref()
            .map(|e| self.lower_expr_with_expected(pkg_prefix, e, init_expected));

        let ty = declared_ty_early
            .or(typechecked_init_ty)
            .or_else(|| init.as_ref().map(|e| e.ty))
            .unwrap_or(self.builtins.any);

        let mut top_level_fqn: Option<String> = None;
        let (id, name) = match v.name() {
            Some(id) => {
                let name = id.text(self.source).to_string();
                let sym = match scope {
                    ValScope::TopLevel => {
                        let fqn = if pkg_prefix.is_empty() {
                            name.clone()
                        } else {
                            format!("{pkg_prefix}.{name}")
                        };
                        top_level_fqn = Some(fqn.clone());
                        self.symbols.intern_top_level(fqn)
                    }
                    ValScope::Local => {
                        self.intern_local_symbol(id.span, v.kind == ast::ValKind::Var)
                    }
                };
                (Some(sym), Some(name))
            }
            None => (None, None),
        };

        if scope == ValScope::TopLevel
            && let Some(fqn) = top_level_fqn.as_ref()
        {
            let extern_symbol = name
                .as_deref()
                .and_then(|default_name| self.extern_global_symbol(v, default_name));
            if let Some(symbol) = extern_symbol.clone() {
                self.extern_globals.insert(
                    fqn.clone(),
                    crate::hir::ExternGlobal {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        mutable: v.kind == ast::ValKind::Var,
                        symbol,
                        linkage: crate::hir::ExternGlobalLinkage::External,
                        storage: self
                            .top_level_var_storage_from_annotations(v)
                            .unwrap_or(TopLevelVarStorage::Global),
                        initializer_absent: v.init.is_none(),
                        unsafe_required: true,
                    },
                );
            } else if v.kind == ast::ValKind::Val {
                self.top_level_immutable_values.insert(
                    fqn.clone(),
                    crate::hir::TopLevelImmutableValue {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        init: init.clone(),
                    },
                );
            }

            // T1023：顶层 `@ThreadLocal/@Global var` 需要后端生成静态存储。
            if extern_symbol.is_none()
                && v.kind == ast::ValKind::Var
                && let Some(storage) = self.top_level_var_storage_from_annotations(v)
            {
                self.top_level_vars.insert(
                    fqn.clone(),
                    crate::hir::TopLevelVar {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        storage,
                        ty,
                        init: init.clone(),
                    },
                );
            }
        }

        ValDecl {
            span: v.span,
            id,
            name,
            mutable: v.kind == ast::ValKind::Var,
            ty,
            init,
        }
    }

    pub(crate) fn extern_global_symbol(
        &self,
        v: &ast::ValDecl,
        default_name: &str,
    ) -> Option<String> {
        v.annotations
            .iter()
            .find_map(|ann| extern_annotation_symbol(self.source, ann, default_name))
    }

    pub(crate) fn top_level_var_storage_from_annotations(
        &self,
        v: &ast::ValDecl,
    ) -> Option<TopLevelVarStorage> {
        const THREAD_LOCAL_FQN: &str = "scoop.core.ThreadLocal";
        const GLOBAL_FQN: &str = "scoop.core.Global";

        if v.annotations
            .iter()
            .any(|ann| self.annotation_use_resolves_to_fqn(ann, THREAD_LOCAL_FQN))
        {
            Some(TopLevelVarStorage::ThreadLocal)
        } else if v
            .annotations
            .iter()
            .any(|ann| self.annotation_use_resolves_to_fqn(ann, GLOBAL_FQN))
        {
            Some(TopLevelVarStorage::Global)
        } else {
            None
        }
    }

    pub(crate) fn annotation_use_resolves_to_fqn(
        &self,
        ann: &ast::AnnotationUse,
        expected_fqn: &str,
    ) -> bool {
        // 与 typecheck 保持一致：复用 Index 的 import/package 解析逻辑，避免仅按未限定名匹配导致误判。
        let ty = ast::TypeRef::Path(ast::TypePath {
            span: ann.span,
            segments: ann.path.clone(),
            args: Vec::new(),
        });

        matches!(
            self.index.type_ref_to_fqn_in_file(self.source, self.file, &ty),
            Some(fqn) if fqn == expected_fqn
        )
    }

    pub(crate) fn lower_type_ref(&mut self, t: &ast::TypeRef) -> TypeId {
        match t {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(tt) => {
                if tt.elements.is_empty() {
                    return self.builtins.unit;
                }
                let elements = tt.elements.iter().map(|e| self.lower_type_ref(e)).collect();
                self.types.ty_tuple(elements)
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner);
                self.types.ty_option(inner)
            }
            ast::TypeRef::Function(fun) => {
                let receiver = fun.receiver.as_ref().map(|r| self.lower_type_ref(r));
                let params = fun.params.iter().map(|p| self.lower_type_ref(p)).collect();
                let return_ty = self.lower_type_ref(&fun.return_ty);
                let effects = self.lower_effect_row_expr(fun.effects.as_ref());
                self.types.ty_function(
                    receiver,
                    params,
                    return_ty,
                    effects,
                    fun.effects.as_ref().is_some_and(|r| r.closed),
                )
            }
            ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => self.builtins.any,
        }
    }

    pub(crate) fn lower_type_path(&mut self, p: &ast::TypePath) -> TypeId {
        // 单段名且无实参：优先解析为当前作用域的 type parameter。
        if p.segments.len() == 1 && p.args.is_empty() {
            let name = p.segments[0].text(self.source);
            if let Some(id) = self.lookup_type_param(name) {
                return id;
            }
        }

        let fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(p.clone()),
        );

        let Some(fqn) = fqn else {
            return self.builtins.any;
        };

        // 分离：普通 type args vs use-site effect row arg（`eff ...`）。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        let mut type_args: Vec<&ast::TypeRef> = Vec::new();
        for a in &p.args {
            match a {
                ast::TypeRef::EffectRowArg { row, .. } => {
                    eff_arg.get_or_insert(row);
                }
                other => type_args.push(other),
            }
        }

        // 少数 builtin/special-case：不走 nominal。
        match fqn.as_str() {
            "scoop.core.Any" => return self.builtins.any,
            "scoop.core.String" => return self.builtins.string,
            "scoop.core.Unit" => return self.builtins.unit,
            "scoop.core.Nothing" => return self.builtins.nothing,
            "scoop.core.Bool" => return self.builtins.bool_,
            "scoop.core.Char" => return self.builtins.char_,
            "scoop.core.Float64" => return self.builtins.float64,
            "scoop.core.Double" => return self.builtins.float64,
            "scoop.core.Float32" => return self.builtins.float32,
            "scoop.core.Int" => return self.builtins.int,
            // T1027：internal atomics（`__AtomicInt`）——与 `Int` 相同布局的内部原子整型。
            "scoop.unsafe.__AtomicInt" => return self.builtins.int,
            "scoop.core.UInt" => return self.builtins.uint,
            "scoop.core.UIntPtr" => return self.builtins.uint,
            "scoop.core.Byte" => return self.types.ty_uint_n(8),
            "scoop.core.Short" => return self.types.ty_int_n(16),
            "scoop.core.UShort" => return self.types.ty_uint_n(16),
            "scoop.core.Long" => return self.types.ty_int_n(64),
            "scoop.core.ULong" => return self.types.ty_uint_n(64),
            "scoop.core.Option" => {
                let inner = type_args
                    .first()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
                return self.types.ty_option(inner);
            }
            _ => {}
        }

        // `Int32`/`UInt64` 这类固定位宽整数：若出现在 sysroot/type env 中，直接 lowering 为内建整数族。
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.Int")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_int_n(bits);
        }
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.UInt")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_uint_n(bits);
        }

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Vec<_>>();
        let eff = eff_arg.map(|e| self.lower_effect_row_expr(Some(e)));
        self.intern_nominal(fqn, args, eff)
    }

    pub(crate) fn lower_effect_row_expr(&mut self, expr: Option<&ast::EffectRowExpr>) -> EffectRow {
        let Some(expr) = expr else {
            return EffectRow::pure();
        };
        if expr.terms.is_empty() {
            return EffectRow::pure();
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            if term.segments.len() == 1 && term.args.is_empty() {
                let name = term.segments[0].text(self.source);
                if let Some(binding) = self
                    .effect_row_param_scopes
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(name))
                {
                    match binding {
                        EffectRowParamBinding::Placeholder(marker) => terms.push(*marker),
                        EffectRowParamBinding::Concrete(row) => {
                            terms.extend(row.terms.iter().copied())
                        }
                    }
                    continue;
                }
            }
            terms.push(self.lower_type_path(term));
        }
        EffectRow::new(terms)
    }

    pub(crate) fn intern_nominal(
        &mut self,
        fqn: String,
        args: Vec<TypeId>,
        eff: Option<EffectRow>,
    ) -> TypeId {
        let nominal = NominalType { fqn, args, eff };

        // 尝试用 `type_kinds` 判断 struct/enum（value type）vs class/interface/effect（ref type）。
        let kind = self.type_kinds.get(&nominal.fqn).copied();
        match kind {
            Some(ast::TypeKind::Struct | ast::TypeKind::Enum) => self
                .types
                .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
            _ => self
                .types
                .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
        }
    }

    pub(crate) fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            self.type_param_scopes.push(HashMap::new());
            return;
        }

        let decl_file = self.source.path().to_path_buf();
        let mut frame = HashMap::new();
        for p in params {
            let name = p.name.text(self.source).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: decl_file.clone(),
                decl_span: p.name.span,
            });
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    /// 直接注入一组“使用点 type param 绑定”（name → TypeId）。
    ///
    /// 用途：
    /// - 单态化实例生成：把 `T` 等抽象类型替换为调用点推断出的具体类型。
    pub(crate) fn push_type_param_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
    ) {
        let mut frame = HashMap::new();
        for (name, id) in bindings {
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    pub(crate) fn type_param_is_bound(&self, name: &str) -> bool {
        self.type_param_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    pub(crate) fn push_missing_type_params(&mut self, params: &[ast::TypeParam]) -> bool {
        let missing = params
            .iter()
            .filter(|param| !self.type_param_is_bound(param.name.text(self.source)))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return false;
        }
        self.push_type_params(&missing);
        true
    }

    pub(crate) fn pop_type_params(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    pub(crate) fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    pub(crate) fn push_synthetic_local_decl_ty(
        &mut self,
        span: Span,
        ty: TypeId,
    ) -> Option<TypeId> {
        self.local_decl_tys.insert(span, ty)
    }

    pub(crate) fn restore_synthetic_local_decl_ty(&mut self, span: Span, previous: Option<TypeId>) {
        if let Some(ty) = previous {
            self.local_decl_tys.insert(span, ty);
        } else {
            self.local_decl_tys.remove(&span);
        }
    }

    pub(in crate::hir::lower) fn synthetic_local_decl_ty(&self, span: Span) -> Option<TypeId> {
        self.local_decl_tys.get(&span).copied()
    }

    pub(crate) fn decl_ast_context(
        &self,
        decl_file: &std::path::Path,
    ) -> Option<(&'a SourceFile, &'a ast::File)> {
        self.compilation_unit
            .iter()
            .copied()
            .find(|(source, _)| source.path() == decl_file)
    }
}
