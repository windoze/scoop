//! Std-delegate parsing, lazy thread-safety, extern libs, calling-convention, CLayout, struct/enum layouts.

#![allow(dead_code)]

use super::*;

pub(in crate::hir::lower) fn object_decl_name(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
) -> Option<String> {
    match obj.name.as_ref() {
        Some(name) => Some(name.text(source).to_string()),
        None => match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        },
    }
}

pub(in crate::hir::lower) fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

#[derive(Debug, Clone)]
pub(in crate::hir::lower) enum ParsedStdDelegateExpr {
    Lazy {
        mode: StdLazyThreadSafetyMode,
        initializer_body: ast::Expr,
    },
    Observable {
        initial: ast::Expr,
        on_change: ast::LambdaExpr,
    },
    Vetoable {
        initial: ast::Expr,
        on_change: ast::LambdaExpr,
    },
    MapBacked {
        delegate: ast::Expr,
    },
}

pub(in crate::hir::lower) fn unique_top_level_fun_fqn_from_callee(
    callee: &ast::Expr,
) -> Option<String> {
    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };

    // 优先使用 resolver 写回的 call candidates（比 `resolved` 更稳健；可覆盖 overload set）。
    if let Some(call) = id.call.as_ref() {
        let mut funs: Vec<String> = call
            .candidates
            .iter()
            .filter_map(|c| match c {
                ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                ast::CallCandidate::Constructor { .. } => None,
            })
            .collect();
        funs.sort();
        funs.dedup();
        if funs.len() == 1 {
            return Some(funs[0].clone());
        }
    }

    // fallback：若 resolver 已把 callee 绑定为唯一顶层函数，同样可用。
    match id.resolved.as_ref() {
        Some(ast::ResolvedValueRef::TopLevel { fqn }) => Some(fqn.clone()),
        _ => None,
    }
}

pub(in crate::hir::lower) fn parse_lazy_thread_safety_mode(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Option<StdLazyThreadSafetyMode> {
    // 目前仅支持最常见的枚举常量写法（用于 delegated property 的 early lowering）：
    // - `LazyThreadSafetyMode.None`
    // - `LazyThreadSafetyMode.Publication`
    // - `LazyThreadSafetyMode.Synchronized`
    //
    // 备注：这里优先从源文本切片解析，避免依赖 enum variant 的 resolver/typecheck 语义细节。
    let raw = source.slice(expr.span).trim();

    // 支持命名参数：`mode = LazyThreadSafetyMode.None`。
    let raw = raw
        .split_once('=')
        .map(|(_, rhs)| rhs.trim())
        .unwrap_or(raw);

    let raw = raw.strip_prefix("scoop.delegates.").unwrap_or(raw);
    match raw {
        "LazyThreadSafetyMode.None" => Some(StdLazyThreadSafetyMode::None),
        "LazyThreadSafetyMode.Publication" => Some(StdLazyThreadSafetyMode::Publication),
        "LazyThreadSafetyMode.Synchronized" => Some(StdLazyThreadSafetyMode::Synchronized),
        _ => None,
    }
}

/// 提取 generic delegated property 的 delegate class FQN，用于 lowered HIR 中 setValue/getValue
/// 调用的 typed call-site contract 发布。常见输入是 `Delegate()` 这样的构造调用，从 resolver
/// 写回的 `Constructor { ty_fqn }` candidate 中读出类的 FQN。
pub(in crate::hir::lower) fn delegate_class_fqn_from_expr(
    delegate_expr: &ast::Expr,
) -> Option<String> {
    let ast::ExprKind::Call { callee, .. } = &delegate_expr.kind else {
        return None;
    };
    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };
    let call = id.call.as_ref()?;
    let mut tys: Vec<String> = call
        .candidates
        .iter()
        .filter_map(|c| match c {
            ast::CallCandidate::Constructor { ty_fqn } => Some(ty_fqn.clone()),
            ast::CallCandidate::Fun { .. } => None,
        })
        .collect();
    tys.sort();
    tys.dedup();
    if tys.len() == 1 {
        Some(tys.remove(0))
    } else {
        None
    }
}

pub(in crate::hir::lower) fn parse_std_delegate_expr(
    source: &SourceFile,
    delegate_expr: &ast::Expr,
) -> Option<ParsedStdDelegateExpr> {
    match &delegate_expr.kind {
        ast::ExprKind::Call { callee, args } => {
            let fqn = unique_top_level_fun_fqn_from_callee(callee)?;

            // lazy：`lazy { ... }` / `lazy(mode) { ... }`
            if fqn == "scoop.delegates.lazy" {
                let last = args.last()?;
                let ast::ExprKind::Lambda(lam) = &last.kind else {
                    return None;
                };

                let mode = if args.len() >= 2 {
                    parse_lazy_thread_safety_mode(source, &args[0])
                        .unwrap_or_else(StdLazyThreadSafetyMode::default_for_lazy_call)
                } else {
                    StdLazyThreadSafetyMode::default_for_lazy_call()
                };
                return Some(ParsedStdDelegateExpr::Lazy {
                    mode,
                    initializer_body: (*lam.body).clone(),
                });
            }

            // observable/vetoable：`observable(init) { old, new -> ... }`
            if fqn == "scoop.delegates.observable" || fqn == "scoop.delegates.vetoable" {
                if args.len() < 2 {
                    return None;
                }
                let initial = args.first()?.clone();
                let last = args.last()?;
                let ast::ExprKind::Lambda(lam) = &last.kind else {
                    return None;
                };

                return if fqn == "scoop.delegates.observable" {
                    Some(ParsedStdDelegateExpr::Observable {
                        initial,
                        on_change: lam.clone(),
                    })
                } else {
                    Some(ParsedStdDelegateExpr::Vetoable {
                        initial,
                        on_change: lam.clone(),
                    })
                };
            }

            None
        }

        // map-backed：`val x: T by data`
        ast::ExprKind::Ident(_) | ast::ExprKind::MemberAccess { .. } => {
            Some(ParsedStdDelegateExpr::MapBacked {
                delegate: delegate_expr.clone(),
            })
        }

        _ => None,
    }
}

pub(in crate::hir::lower) fn collect_delegated_properties<'a>(
    pairs: &[(&'a SourceFile, &'a ast::File)],
) -> DelegatedPropertyIndex<'a> {
    let mut out: DelegatedPropertyIndex<'a> = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_delegated_properties_in_type_decl(
                        source,
                        file,
                        ty,
                        &pkg_prefix,
                        &mut out,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_delegated_properties_in_object_decl(
                        source,
                        file,
                        obj,
                        &pkg_prefix,
                        &mut out,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    out
}

pub(in crate::hir::lower) fn collect_delegated_properties_in_type_decl<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut DelegatedPropertyIndex<'a>,
) {
    let local_name = source.slice(decl.name.span);
    let owner_fqn = join_prefix(prefix, local_name);

    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) if p.delegate.is_some() => {
                    let name = source.slice(p.name.span).to_string();
                    let prop_fqn = format!("{owner_fqn}.{name}");
                    let Some(delegate_expr) = p.delegate.as_ref() else {
                        continue;
                    };

                    let info = match parse_std_delegate_expr(source, delegate_expr) {
                        Some(ParsedStdDelegateExpr::Lazy {
                            mode,
                            initializer_body,
                        }) => {
                            let mutex_field_fqn = mode
                                .requires_mutex()
                                .then(|| format!("{owner_fqn}.{name}$lazy_mutex"));
                            DelegatedPropertyInfo::Lazy(LazyDelegatedPropertyInfo {
                                decl: DelegatedPropertyDeclContext { source, file },
                                name: name.clone(),
                                ty: p.ty.clone(),
                                mode,
                                value_field_fqn: format!("{owner_fqn}.{name}$lazy_value"),
                                inited_field_fqn: format!("{owner_fqn}.{name}$lazy_inited"),
                                mutex_field_fqn,
                                initializer_body,
                            })
                        }
                        Some(ParsedStdDelegateExpr::Observable { on_change, .. }) => {
                            let mutex_field_fqn =
                                Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                            DelegatedPropertyInfo::Observable(ObservableDelegatedPropertyInfo {
                                decl: DelegatedPropertyDeclContext { source, file },
                                name: name.clone(),
                                property_fqn: prop_fqn.clone(),
                                ty: p.ty.clone(),
                                on_change,
                                mutex_field_fqn,
                            })
                        }
                        Some(ParsedStdDelegateExpr::Vetoable { on_change, .. }) => {
                            let mutex_field_fqn =
                                Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                            DelegatedPropertyInfo::Vetoable(VetoableDelegatedPropertyInfo {
                                decl: DelegatedPropertyDeclContext { source, file },
                                name: name.clone(),
                                property_fqn: prop_fqn.clone(),
                                ty: p.ty.clone(),
                                on_change,
                                mutex_field_fqn,
                            })
                        }
                        Some(ParsedStdDelegateExpr::MapBacked { .. }) => {
                            DelegatedPropertyInfo::MapBacked
                        }
                        None => {
                            let delegate_field_fqn = format!("{owner_fqn}.{name}$delegate");
                            let property_meta_fqn = format!("{owner_fqn}.$PropertyMeta${name}");
                            let delegate_class_fqn = delegate_class_fqn_from_expr(delegate_expr);
                            DelegatedPropertyInfo::Generic(GenericDelegatedPropertyInfo {
                                name: name.clone(),
                                delegate_field_fqn,
                                property_meta_fqn,
                                delegate_class_fqn,
                            })
                        }
                    };

                    out.entry(prop_fqn).or_insert(info);
                }
                ast::TypeMember::Type(nested) => {
                    collect_delegated_properties_in_type_decl(
                        source, file, nested, &owner_fqn, out,
                    );
                }
                ast::TypeMember::Object(obj) => {
                    collect_delegated_properties_in_object_decl(source, file, obj, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }
}

pub(in crate::hir::lower) fn collect_delegated_properties_in_object_decl<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut DelegatedPropertyIndex<'a>,
) {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => return,
        },
    };

    let owner_fqn = join_prefix(prefix, &obj_name);
    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) if p.delegate.is_some() => {
                let name = source.slice(p.name.span).to_string();
                let prop_fqn = format!("{owner_fqn}.{name}");
                let Some(delegate_expr) = p.delegate.as_ref() else {
                    continue;
                };

                let info = match parse_std_delegate_expr(source, delegate_expr) {
                    Some(ParsedStdDelegateExpr::Lazy {
                        mode,
                        initializer_body,
                    }) => {
                        let mutex_field_fqn = mode
                            .requires_mutex()
                            .then(|| format!("{owner_fqn}.{name}$lazy_mutex"));
                        DelegatedPropertyInfo::Lazy(LazyDelegatedPropertyInfo {
                            decl: DelegatedPropertyDeclContext { source, file },
                            name: name.clone(),
                            ty: p.ty.clone(),
                            mode,
                            value_field_fqn: format!("{owner_fqn}.{name}$lazy_value"),
                            inited_field_fqn: format!("{owner_fqn}.{name}$lazy_inited"),
                            mutex_field_fqn,
                            initializer_body,
                        })
                    }
                    Some(ParsedStdDelegateExpr::Observable { on_change, .. }) => {
                        let mutex_field_fqn = Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                        DelegatedPropertyInfo::Observable(ObservableDelegatedPropertyInfo {
                            decl: DelegatedPropertyDeclContext { source, file },
                            name: name.clone(),
                            property_fqn: prop_fqn.clone(),
                            ty: p.ty.clone(),
                            on_change,
                            mutex_field_fqn,
                        })
                    }
                    Some(ParsedStdDelegateExpr::Vetoable { on_change, .. }) => {
                        let mutex_field_fqn = Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                        DelegatedPropertyInfo::Vetoable(VetoableDelegatedPropertyInfo {
                            decl: DelegatedPropertyDeclContext { source, file },
                            name: name.clone(),
                            property_fqn: prop_fqn.clone(),
                            ty: p.ty.clone(),
                            on_change,
                            mutex_field_fqn,
                        })
                    }
                    Some(ParsedStdDelegateExpr::MapBacked { .. }) => {
                        DelegatedPropertyInfo::MapBacked
                    }
                    None => {
                        let delegate_field_fqn = format!("{owner_fqn}.{name}$delegate");
                        let property_meta_fqn = format!("{owner_fqn}.$PropertyMeta${name}");
                        let delegate_class_fqn = delegate_class_fqn_from_expr(delegate_expr);
                        DelegatedPropertyInfo::Generic(GenericDelegatedPropertyInfo {
                            name: name.clone(),
                            delegate_field_fqn,
                            property_meta_fqn,
                            delegate_class_fqn,
                        })
                    }
                };

                out.entry(prop_fqn).or_insert(info);
            }
            ast::TypeMember::Type(nested) => {
                collect_delegated_properties_in_type_decl(source, file, nested, &owner_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_delegated_properties_in_object_decl(source, file, nested, &owner_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_extern_funs(
    source: &SourceFile,
    file: &ast::File,
) -> ExternFunIndex {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out: ExternFunIndex = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let Some(extern_fun) = extern_fun_of_decl(source, fun) else {
            continue;
        };

        let name = fun.name.text(source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name
        } else {
            format!("{pkg_prefix}.{name}")
        };

        out.insert(fqn, extern_fun);
    }

    out
}

pub(in crate::hir::lower) fn collect_native_callable_funs(
    source: &SourceFile,
    file: &ast::File,
) -> NativeCallableFunIndex {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out: NativeCallableFunIndex = HashMap::new();

    for item in &file.items {
        match item {
            ast::Item::Fun(fun) => {
                collect_native_callable_fun_decl(source, fun, &pkg_prefix, &mut out);
            }
            ast::Item::Type(decl) => {
                collect_native_callable_funs_in_type_decl(source, decl, &pkg_prefix, &mut out);
            }
            ast::Item::Object(obj) => {
                collect_native_callable_funs_in_object_decl(source, obj, &pkg_prefix, &mut out);
            }
            ast::Item::TypeAlias(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::Val(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    out
}

fn collect_native_callable_funs_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    owner_prefix: &str,
    out: &mut NativeCallableFunIndex,
) {
    let owner_fqn = join_prefix(owner_prefix, decl.name.text(source));
    let Some(body) = &decl.body else { return };

    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => {
                collect_native_callable_fun_decl(source, fun, &owner_fqn, out);
            }
            ast::TypeMember::Type(nested) => {
                collect_native_callable_funs_in_type_decl(source, nested, &owner_fqn, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_native_callable_funs_in_object_decl(source, obj, &owner_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

fn collect_native_callable_funs_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    owner_prefix: &str,
    out: &mut NativeCallableFunIndex,
) {
    let Some(obj_name) = object_decl_name(source, obj) else {
        return;
    };
    let owner_fqn = join_prefix(owner_prefix, &obj_name);
    let Some(body) = &obj.body else { return };

    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => {
                collect_native_callable_fun_decl(source, fun, &owner_fqn, out);
            }
            ast::TypeMember::Type(nested) => {
                collect_native_callable_funs_in_type_decl(source, nested, &owner_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_native_callable_funs_in_object_decl(source, nested, &owner_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

fn collect_native_callable_fun_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
    owner_prefix: &str,
    out: &mut NativeCallableFunIndex,
) {
    let name = fun.name.text(source).to_string();
    let Some(native_callable) = native_callable_fun_of_decl(source, fun, &name) else {
        return;
    };
    out.insert(join_prefix(owner_prefix, &name), native_callable);
}

#[derive(Debug, Default, Clone)]
pub(in crate::hir::lower) struct ExternAnnotationArgs {
    pub(in crate::hir::lower) name: Option<String>,
    pub(in crate::hir::lower) lib: Option<String>,
    pub(in crate::hir::lower) abi: ExternAbi,
    pub(in crate::hir::lower) calling_convention: Option<String>,
}

pub(in crate::hir::lower) fn parse_extern_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> ExternAnnotationArgs {
    let mut out = ExternAnnotationArgs::default();
    let mut seen_named = false;

    for arg in &ann.args {
        // 兼容两种“命名参数”形态：
        // - `name: "..."`（AnnotationArg.name）
        // - `name = "..."`（赋值表达式；更贴近 Kotlin 风格）
        let (key, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, None),
            },
        };

        if let (Some(key), Some(value)) = (key, value) {
            seen_named = true;
            if !matches!(value.kind, ast::ExprKind::StringLit) {
                continue;
            }
            let text = source.slice(value.span);
            match key {
                "name" => out.name = parse_string_literal_utf8(text).ok(),
                "lib" => out.lib = parse_string_literal_utf8(text).ok(),
                "abi" => {
                    if let Ok(name) = parse_string_literal_utf8(text)
                        && let Some(abi) = ExternAbi::parse(&name)
                    {
                        out.abi = abi;
                    }
                }
                "callingConvention" => {
                    out.calling_convention = parse_string_literal_utf8(text).ok();
                }
                _ => {}
            }
            continue;
        }

        // 位置参数：`@Extern("symbol")`（仅在未出现命名参数前生效）。
        if seen_named {
            continue;
        }
        if out.name.is_some() {
            continue;
        }
        if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
            continue;
        }
        let text = source.slice(arg.value.span);
        out.name = parse_string_literal_utf8(text).ok();
    }

    out
}

pub(in crate::hir::lower) fn extern_fun_of_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
) -> Option<ExternFun> {
    // 说明：
    // - `@Extern` 在语义上由 typecheck 校验（参数个数/类型等）；
    // - HIR lowering 只做“提取已校验信息”的 best-effort，避免把错误传播面扩到 HIR/LLVM 层。
    let name = fun.name.text(source);
    for ann in &fun.annotations {
        if !is_builtin_extern_annotation(source, ann) {
            continue;
        }

        // `@Extern`：缺省用函数名作为链接符号名；若显式提供 `name = "..."`（或位置参数），则覆写。
        let args = parse_extern_annotation_args(source, ann);
        let symbol = args.name.unwrap_or_else(|| name.to_string());

        return Some(ExternFun {
            abi: args.abi,
            symbol,
            calling_convention: args.calling_convention,
            lib: args.lib,
        });
    }

    None
}

pub(in crate::hir::lower) fn native_callable_fun_of_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
    default_symbol: &str,
) -> Option<NativeCallableFun> {
    for ann in &fun.annotations {
        if !is_builtin_calling_convention_annotation(source, ann) {
            continue;
        }

        let args = parse_calling_convention_annotation_args(source, ann);
        let calling_convention = args.convention?;
        let symbol = args.symbol.unwrap_or_else(|| default_symbol.to_string());
        return Some(NativeCallableFun {
            symbol,
            calling_convention,
        });
    }

    None
}

pub(in crate::hir::lower) fn extern_annotation_symbol(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    default_name: &str,
) -> Option<String> {
    if !is_builtin_extern_annotation(source, ann) {
        return None;
    }
    let args = parse_extern_annotation_args(source, ann);
    Some(args.name.unwrap_or_else(|| default_name.to_string()))
}

pub(in crate::hir::lower) fn collect_extern_libs(
    pairs: &[(&SourceFile, &ast::File)],
) -> Vec<String> {
    let mut libs: HashSet<String> = HashSet::new();

    for (source, file) in pairs {
        collect_extern_libs_in_file(source, file, &mut libs);
    }

    let mut out = libs.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

pub(in crate::hir::lower) fn collect_extern_libs_in_file(
    source: &SourceFile,
    file: &ast::File,
    out: &mut HashSet<String>,
) {
    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                collect_extern_libs_in_annotations(source, &ta.annotations, out);
            }
            ast::Item::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out);
            }
            ast::Item::ExtensionProperty(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out);
            }
            ast::Item::Val(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out);
            }
            ast::Item::Type(ty) => {
                collect_extern_libs_in_type_decl(source, ty, out);
            }
            ast::Item::Object(obj) => {
                collect_extern_libs_in_object_decl(source, obj, out);
            }
            // T1220a：package-level comptime if 在进入后续阶段前应被裁剪（TODO T1220b）。
            ast::Item::ComptimeIf(_ci) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_extern_libs_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    out: &mut HashSet<String>,
) {
    collect_extern_libs_in_annotations(source, &decl.annotations, out);

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out)
            }
            ast::TypeMember::Property(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out)
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                collect_extern_libs_in_annotations(source, &ctor.annotations, out);
                for p in &ctor.params {
                    collect_extern_libs_in_annotations(source, &p.annotations, out);
                }
            }
            ast::TypeMember::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out)
            }
            ast::TypeMember::Type(nested) => collect_extern_libs_in_type_decl(source, nested, out),
            ast::TypeMember::Object(obj) => collect_extern_libs_in_object_decl(source, obj, out),
        }
    }
}

pub(in crate::hir::lower) fn collect_extern_libs_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    out: &mut HashSet<String>,
) {
    collect_extern_libs_in_annotations(source, &obj.annotations, out);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out)
            }
            ast::TypeMember::Property(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out)
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                collect_extern_libs_in_annotations(source, &ctor.annotations, out);
                for p in &ctor.params {
                    collect_extern_libs_in_annotations(source, &p.annotations, out);
                }
            }
            ast::TypeMember::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out)
            }
            ast::TypeMember::Type(nested) => collect_extern_libs_in_type_decl(source, nested, out),
            ast::TypeMember::Object(nested) => {
                collect_extern_libs_in_object_decl(source, nested, out)
            }
        }
    }
}

pub(in crate::hir::lower) fn collect_extern_libs_in_annotations(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    out: &mut HashSet<String>,
) {
    for ann in annotations {
        if !is_builtin_extern_annotation(source, ann) {
            continue;
        }
        let args = parse_extern_annotation_args(source, ann);
        if let Some(lib) = args.lib
            && !lib.is_empty()
        {
            out.insert(lib);
        }
    }
}

pub(in crate::hir::lower) fn is_builtin_extern_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> bool {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    matches!(segs.as_slice(), ["Extern"] | ["scoop", "core", "Extern"])
}

pub(in crate::hir::lower) fn is_builtin_calling_convention_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> bool {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    matches!(
        segs.as_slice(),
        ["CallingConvention"] | ["scoop", "core", "CallingConvention"]
    )
}

#[derive(Debug, Default, Clone)]
pub(in crate::hir::lower) struct CallingConventionAnnotationArgs {
    pub(in crate::hir::lower) symbol: Option<String>,
    pub(in crate::hir::lower) convention: Option<String>,
}

pub(in crate::hir::lower) fn parse_calling_convention_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> CallingConventionAnnotationArgs {
    let mut out = CallingConventionAnnotationArgs::default();
    let mut seen_named = false;

    for arg in &ann.args {
        let (key, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, Some(&arg.value)),
            },
        };

        if let (Some(key), Some(value)) = (key, value) {
            seen_named = true;
            if !matches!(value.kind, ast::ExprKind::StringLit) {
                continue;
            }
            let text = source.slice(value.span);
            match key {
                "name" => out.symbol = parse_string_literal_utf8(text).ok(),
                "convention" => out.convention = parse_string_literal_utf8(text).ok(),
                _ => {}
            }
            continue;
        }

        if seen_named
            || out.convention.is_some()
            || !matches!(arg.value.kind, ast::ExprKind::StringLit)
        {
            continue;
        }
        let text = source.slice(arg.value.span);
        out.convention = parse_string_literal_utf8(text).ok();
    }

    out
}

pub(in crate::hir::lower) fn parse_calling_convention_annotation_arg(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Option<String> {
    let arg = ann.args.first()?;

    // 兼容两种“命名参数”形态：
    // - `name: "..."`（AnnotationArg.name）
    // - `name = "..."`（赋值表达式；更贴近 Kotlin 风格）
    let (key, value) = match &arg.name {
        Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
        None => match &arg.value.kind {
            ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                _ => (None, None),
            },
            _ => (None, Some(&arg.value)),
        },
    };

    if let Some(key) = key
        && key != "name"
    {
        return None;
    }

    if !matches!(value?.kind, ast::ExprKind::StringLit) {
        return None;
    }

    let text = source.slice(value?.span);
    parse_string_literal_utf8(text).ok()
}

pub(in crate::hir::lower) fn annotation_use_resolves_to_fqn_in_file(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ann: &ast::AnnotationUse,
    expected_fqn: &str,
) -> bool {
    // 与 typecheck 阶段一致：复用 Index 的“按 package/import 规则解析类型名”的逻辑，
    // 避免仅按未限定名匹配导致的误判（同名但不同包的注解类）。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });
    matches!(
        index.type_ref_to_fqn_in_file(source, file, &ty),
        Some(fqn) if fqn == expected_fqn
    )
}

pub(in crate::hir::lower) fn extract_struct_clayout(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    annotations: &[ast::AnnotationUse],
) -> Option<StructCLayout> {
    const CLAYOUT_FQN: &str = "scoop.core.CLayout";
    let ann = annotations.iter().find(|ann| {
        annotation_use_resolves_to_fqn_in_file(source, file, index, ann, CLAYOUT_FQN)
    })?;

    Some(parse_clayout_annotation_args(source, ann))
}

pub(in crate::hir::lower) fn parse_clayout_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> StructCLayout {
    // 说明：
    // - HIR lowering（dump/fixtures）不运行完整 typecheck，因此这里按 best-effort 解析；
    // - 实参的合法性与 GC-free 约束由 typecheck 阶段负责；
    // - 这里仅做“形态提取”，供后端在 typecheck 成功后消费。
    let mut aligned: Option<u32> = None;
    let mut packed: Option<u32> = None;

    for (pos, arg) in ann.args.iter().enumerate() {
        // 兼容三种参数形态（与 `@Extern` 一致）：
        // - `aligned: 16`（AnnotationArg.name）
        // - `aligned = 16`（赋值表达式；更贴近 Kotlin 风格）
        // - 位置参数：`@CLayout(16, 1)`（按顺序映射到 aligned/packed）
        let (key, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, Some(&arg.value)),
            },
        };

        let key = match key {
            Some(key) => key,
            None => match pos {
                0 => "aligned",
                1 => "packed",
                _ => continue,
            },
        };
        let Some(value) = value else { continue };

        let ast::ExprKind::IntLit = value.kind else {
            continue;
        };
        let raw = source.slice(value.span);
        let Some(v) = parse_int_literal_u32(raw) else {
            continue;
        };
        let v = if v == 0 { None } else { Some(v) };

        match key {
            "aligned" => aligned = v,
            "packed" => packed = v,
            _ => {}
        }
    }

    StructCLayout { aligned, packed }
}

pub(in crate::hir::lower) fn parse_int_literal_u32(text: &str) -> Option<u32> {
    u32::try_from(parse_int_literal(text)).ok()
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 struct 字段布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 struct**；
/// - 仅使用 struct 的 primary ctor params 作为字段（与 resolver 对齐：`p.x` 来自 ctor param）；
/// - 暂不支持泛型 struct / `eff` 参数化 struct：这类布局需要单态化后再确定（留到后续任务）。
pub(in crate::hir::lower) fn collect_struct_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    types: &mut TypeStore,
) -> StructLayoutIndex {
    let mut out: StructLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Struct) {
                continue;
            }

            // 泛型/eff 参数化 struct 的布局需要在 monomorphization 后才能稳定确定：
            // - field 的 type args 可能包含未绑定的 type params；
            // - ABI/layout 可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 避免重复写入（例如 sysroot 与用户文件存在同名 type 时，resolver 会先报错）。
            if out.contains_key(&fqn) {
                continue;
            }

            let c_layout = extract_struct_clayout(source, file, index, &ty.annotations);

            let mut fields: Vec<StructFieldLayout> = Vec::new();
            if let Some(primary_ctor) = &ty.primary_ctor {
                for p in &primary_ctor.params {
                    let ty = p
                        .ty
                        .as_ref()
                        .and_then(|t| type_ref_to_layout_type_id(source, file, index, t, types));
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));
                    push_struct_layout_field(
                        &mut fields,
                        &fqn,
                        p.name.span,
                        p.name.text(source).to_string(),
                        ty,
                        ty_fqn,
                    );
                }
            }
            append_struct_body_property_layout_fields(
                source,
                ty.body.as_ref(),
                &fqn,
                |ty_ref| {
                    (
                        index.type_ref_to_fqn_in_file(source, file, ty_ref),
                        type_ref_to_layout_type_id(source, file, index, ty_ref, types),
                    )
                },
                &mut fields,
            );

            out.insert(
                fqn.clone(),
                StructLayout {
                    fqn,
                    fields,
                    c_layout,
                },
            );
        }
    }

    out
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 enum variant 布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 enum**；
/// - 暂不支持泛型 enum / `eff` 参数化 enum（这类布局需要单态化后再确定，留到后续任务）；
/// - variant tag 按声明顺序分配，从 0 开始（与 typecheck/type env 的最小规则对齐）。
pub(in crate::hir::lower) fn collect_enum_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    types: &mut TypeStore,
) -> EnumLayoutIndex {
    let mut out: EnumLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Enum) {
                continue;
            }

            // 泛型/eff 参数化 enum 的布局需要在 monomorphization 后才能稳定确定：
            // - payload 字段类型可能包含未绑定的 type params；
            // - 后端布局/boxing 策略可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            if out.contains_key(&fqn) {
                continue;
            }

            let mut variants: Vec<EnumVariantLayout> = Vec::new();
            let mut repr: EnumRepr = EnumRepr::TaggedUnion;

            let Some(body) = &ty.body else {
                out.insert(
                    fqn.clone(),
                    EnumLayout {
                        fqn,
                        repr,
                        variants,
                    },
                );
                continue;
            };

            // spec §2.3.2.1：value-only enum。
            //
            // 当前阶段的判定策略（避免与 “enum implements interfaces” 的 `:` 语法冲突）：
            // - 只有当 enum body 内出现了显式判别值（`A = 0`）时，才把第一个 supertype 视为底层整型表示。
            if !ty.supertypes.is_empty()
                && body.members.iter().any(
                    |m| matches!(m, ast::TypeMember::EnumVariant(v) if v.discriminant.is_some()),
                )
            {
                let underlying_ty_fqn = ty
                    .supertypes
                    .first()
                    .and_then(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty));
                repr = EnumRepr::ValueOnly { underlying_ty_fqn };
            }

            let mut next_tag: u64 = 0;
            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };

                let variant_name = v.name.text(source).to_string();
                let tag = match repr {
                    EnumRepr::TaggedUnion => {
                        let tag = next_tag;
                        next_tag = next_tag.saturating_add(1);
                        tag
                    }
                    EnumRepr::ValueOnly { .. } => v
                        .discriminant
                        .as_ref()
                        .and_then(|e| eval_value_only_enum_discriminant(source, e))
                        .map(|v| v as u64)
                        .unwrap_or_else(|| {
                            let tag = next_tag;
                            next_tag = next_tag.saturating_add(1);
                            tag
                        }),
                };

                let mut fields: Vec<EnumVariantFieldLayout> = Vec::new();
                for p in &v.params {
                    let field_name = p.name.text(source).to_string();
                    let ty = p
                        .ty
                        .as_ref()
                        .and_then(|t| type_ref_to_layout_type_id(source, file, index, t, types));
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));
                    fields.push(EnumVariantFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        ty,
                        ty_fqn,
                    });
                }

                variants.push(EnumVariantLayout {
                    span: v.span,
                    name: variant_name,
                    tag,
                    fields,
                });
            }

            out.insert(
                fqn.clone(),
                EnumLayout {
                    fqn,
                    repr,
                    variants,
                },
            );
        }
    }

    out
}
