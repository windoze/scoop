//! 从编译器内部结构导出 ScoopIR（v0）。
//!
//! 输入来源：
//! - `TypeEnv`：提供类型声明头（kind、type params 等）
//! - `HIR + TypeStore`：提供函数签名的 `TypeId`（已 lower/解析）
//! - `Index`：提供可见性信息（只导出 `public`）

use std::collections::HashSet;

use miette::Diagnostic;
use thiserror::Error;

use crate::hir::{Item as HirItem, LoweredHir};
use crate::resolve::{Index, Visibility};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, FunctionType, NominalType, RefTypeKind, TypeId, TypeKind,
    TypeParamType, TypeStore, UnionType, ValueTypeKind,
};
use crate::typecheck::{
    AnnotationRetentionPolicy, TypeEnv, TypeLowering, TypeSymbol, TypeSymbolKind,
};

use super::schema::{
    IrEffectRow, IrFunDecl, IrFunDeclKind, IrFunParam, IrType, IrTypeDecl, IrTypeDeclKind,
    IrTypeParam, IrVariance, ScoopIrFile,
};

#[derive(Debug, Error, Diagnostic)]
pub enum ScoopIrExportError {
    #[error("无法在 TypeEnv 中查询类型符号：{fqn}")]
    #[diagnostic(code(scoop::scoopir::missing_type_symbol))]
    MissingTypeSymbol { fqn: String },

    #[error("无法在 TypeEnv 中查询 typealias 声明信息：{fqn}")]
    #[diagnostic(code(scoop::scoopir::missing_typealias_info))]
    MissingTypeAliasInfo { fqn: String },

    #[error("导出 typealias 失败：无法 lowering RHS：{fqn}")]
    #[diagnostic(code(scoop::scoopir::typealias_rhs_lowering_failed))]
    TypeAliasRhsLoweringFailed {
        fqn: String,
        #[source]
        #[diagnostic_source]
        source: crate::typecheck::TypeLowerError,
    },

    #[error(
        "导出 typealias 失败：public typealias {alias_fqn} 的 RHS 引用了非 public 类型：{referenced_fqn}"
    )]
    #[diagnostic(code(scoop::scoopir::typealias_exposes_non_public_type))]
    TypeAliasExposesNonPublicType {
        alias_fqn: String,
        referenced_fqn: String,
    },

    #[error(
        "导出 typealias 失败：public typealias {alias_fqn} 的 RHS 引用了未知类型：{referenced_fqn}"
    )]
    #[diagnostic(code(scoop::scoopir::typealias_references_unknown_type))]
    TypeAliasReferencesUnknownType {
        alias_fqn: String,
        referenced_fqn: String,
    },

    #[error("HIR 中函数类型不是 function type：{fqn}")]
    #[diagnostic(code(scoop::scoopir::unexpected_fun_type))]
    UnexpectedFunType { fqn: String },

    #[error("无法在 Index 中查询函数符号：{fqn}")]
    #[diagnostic(code(scoop::scoopir::missing_fun_symbol))]
    MissingFunSymbol { fqn: String },
}

/// 从“单个源文件”的编译产物导出其 public API 的 ScoopIR v0。
///
/// 说明：
/// - 目前按 `decl_file == source.path()` 过滤“属于该文件”的声明；
/// - `.cone` 的“跨文件/跨包”聚合由后续任务（T1104/T1107）接入。
pub fn export_public_api_for_source(
    source: &SourceFile,
    index: &Index,
    env: &TypeEnv,
    hir: &LoweredHir,
) -> Result<ScoopIrFile, ScoopIrExportError> {
    let mut types = export_public_types_for_source(source, index, env)?;
    types.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    let mut funs = export_public_funs_for_source(source, index, hir)?;
    funs.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    Ok(ScoopIrFile::new_v0(types, funs))
}

/// 从一个 cone 的多个源文件中导出其 public API 的 ScoopIR v0（聚合版）。
///
/// 说明（v0 约束）：
/// - 目前只导出 public type/fun 的“声明头”；不导出函数体；
/// - 该函数会构建 sysroot + 当前 cone 的 `Index` 并运行 resolver（headers + bodies），
///   以确保 HIR lowering 能得到稳定的 `TypeId`；
/// - `.cone` 的 IR 版本协商与下游消费由后续任务实现（TODO T1105/T1106）。
pub fn export_public_api_for_cone_sources(
    session: &Session,
    sources: &[SourceFile],
) -> miette::Result<ScoopIrFile> {
    if sources.is_empty() {
        return Err(miette::miette!("导出 ScoopIR 失败：sources 为空"));
    }

    // 1) parse：得到 AST（后续 resolver 会写回绑定结果，因此这里用可变 Vec 承载）。
    let mut asts = Vec::with_capacity(sources.len());
    for source in sources {
        let mut ast = crate::parser::parse_file(source).map_err(miette::Report::from)?;
        crate::comptime::trim_package_level_comptime_ifs(source, &mut ast)
            .map_err(miette::Report::from)?;
        asts.push(ast);
    }

    // 2) index：sysroot cone=0，当前 cone=1（与 `scoop build` 对齐）。
    let mut indexed: Vec<crate::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(crate::resolve::IndexedFile {
            cone: crate::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(crate::resolve::IndexedFile {
            cone: crate::resolve::ConeId::new(1),
            source,
            file: ast,
        });
    }

    let index = crate::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // 3) resolver：headers + bodies，把绑定结果写回 AST（供 HIR lowering 使用）。
    let mut headers = Vec::with_capacity(sources.len());
    for (source, ast) in sources.iter().zip(asts.iter()) {
        let h = crate::resolve::check_file_headers(source, ast, &index)
            .map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in sources.iter().zip(asts.iter_mut()).zip(headers.iter()) {
        crate::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    // 4) type env：用于导出 public type 的声明头信息。
    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    // 5) HIR lowering + per-file export，再聚合。
    let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        pairs.push((source, ast));
    }

    let mut all_types = Vec::new();
    let mut all_funs = Vec::new();
    for (source, ast) in sources.iter().zip(asts.iter()) {
        let hir = crate::hir::lower_for_compilation_unit(source, ast, &index, &pairs)
            .map_err(miette::Report::from)?;
        let ir = export_public_api_for_source(source, &index, &env, &hir)
            .map_err(miette::Report::from)?;

        all_types.extend(ir.types);
        all_funs.extend(ir.funs);
    }

    // 排序 + 去重（理论上 resolver/index 已保证 FQN 唯一，这里做防御性处理）。
    all_types.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    all_types.dedup_by(|a, b| a.fqn == b.fqn);

    all_funs.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    all_funs.dedup_by(|a, b| a.fqn == b.fqn);

    Ok(ScoopIrFile::new_v0(all_types, all_funs))
}

/// 收集某个源文件内声明的 `public` 类型（type header）。
fn export_public_types_for_source(
    source: &SourceFile,
    index: &Index,
    env: &TypeEnv,
) -> Result<Vec<IrTypeDecl>, ScoopIrExportError> {
    let mut out: Vec<IrTypeDecl> = Vec::new();

    for (fqn, ns) in &index.by_fqn {
        let Some(sym) = &ns.ty else {
            continue;
        };
        if sym.decl_file.as_path() != source.path() {
            continue;
        }
        if sym.visibility != Visibility::Public {
            continue;
        }

        let symbol = env
            .type_symbol(fqn)
            .ok_or_else(|| ScoopIrExportError::MissingTypeSymbol { fqn: fqn.clone() })?;

        // T1016b：comptime-only annotation classes 不导出到 `.cone` 的 public API。
        //
        // 说明：
        // - annotation class 默认视为 comptime-only，只有显式标记 `@Retention("cone")`
        //   才会跨 cone 导出；
        // - 该过滤会让下游 `.cone` 依赖无法引用 comptime-only 注解类（符合“不可见”的语义边界）。
        if symbol.is_annotation_class
            && symbol.annotation_retention != Some(AnnotationRetentionPolicy::ConePreserved)
        {
            continue;
        }

        let kind = match symbol.kind {
            TypeSymbolKind::Nominal(k) => match k {
                crate::ast::TypeKind::Class => IrTypeDeclKind::Class,
                crate::ast::TypeKind::Interface => IrTypeDeclKind::Interface,
                crate::ast::TypeKind::Struct => IrTypeDeclKind::Struct,
                crate::ast::TypeKind::Enum => IrTypeDeclKind::Enum,
                crate::ast::TypeKind::Effect => IrTypeDeclKind::Effect,
            },
            TypeSymbolKind::TypeAlias => IrTypeDeclKind::TypeAlias,
        };

        let type_params = symbol
            .type_param_names
            .iter()
            .zip(symbol.type_param_variances.iter().copied())
            .map(|(name, variance)| IrTypeParam {
                name: name.clone(),
                variance: variance.map(|v| match v {
                    crate::ast::TypeParamVariance::In => IrVariance::In,
                    crate::ast::TypeParamVariance::Out => IrVariance::Out,
                }),
            })
            .collect::<Vec<_>>();

        out.push(IrTypeDecl {
            fqn: fqn.clone(),
            kind,
            type_params,
            alias_of: match symbol.kind {
                TypeSymbolKind::TypeAlias => {
                    Some(export_type_alias_rhs_ir_type(index, env, symbol, fqn)?)
                }
                _ => None,
            },
        });
    }

    Ok(out)
}

/// 为 public `typealias` 导出其 RHS 类型表达式（ScoopIR IrType）。
///
/// v0 策略（T1302）：
/// - 在导出侧把 RHS lowering 到 `TypeId` 再转换为 `IrType`；
/// - 这样 RHS 会按 typecheck 的别名展开策略被规范化（避免把 internal alias 链暴露到下游）；
/// - 同时校验 public typealias 的 RHS 不得引用非 public 类型，否则下游无法通过 `.cone` 使用该别名。
fn export_type_alias_rhs_ir_type(
    index: &Index,
    env: &TypeEnv,
    alias_sym: &TypeSymbol,
    alias_fqn: &str,
) -> Result<IrType, ScoopIrExportError> {
    let info =
        env.type_alias(alias_fqn)
            .ok_or_else(|| ScoopIrExportError::MissingTypeAliasInfo {
                fqn: alias_fqn.to_string(),
            })?;

    let decl_source =
        env.source(&info.decl_file)
            .ok_or_else(|| ScoopIrExportError::MissingTypeSymbol {
                fqn: alias_fqn.to_string(),
            })?;

    let (pkg_prefix, imports) = env
        .file_type_context(&info.decl_file)
        .map(|ctx| (ctx.pkg_prefix.clone(), ctx.imports.clone()))
        .unwrap_or_else(|| (String::new(), crate::resolve::ImportTable::default()));

    // 为 alias 的 type params 构造占位的 param TypeId（用于 RHS lowering 时识别 `T`）。
    let decl_span = Span::new(0, 0);
    let mut types = TypeStore::new();
    let builtins: BuiltinTypes = types.intern_builtins();
    let param_ids = alias_sym
        .type_param_names
        .iter()
        .map(|name| {
            types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: info.decl_file.clone(),
                decl_span,
            })
        })
        .collect::<Vec<_>>();
    let bindings = alias_sym
        .type_param_names
        .iter()
        .cloned()
        .zip(param_ids.iter().copied())
        .collect::<Vec<_>>();

    let rhs_id = {
        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            index,
            env,
            &mut types,
            builtins,
            pkg_prefix,
            imports,
        );

        ctx.push_type_param_bindings(bindings);
        let out = ctx.lower_type_ref(&info.ty);
        ctx.pop_type_param_bindings();

        out.map_err(|source| ScoopIrExportError::TypeAliasRhsLoweringFailed {
            fqn: alias_fqn.to_string(),
            source,
        })?
    };

    let ir = to_ir_type(&types, rhs_id, 0);
    assert_typealias_rhs_is_public(index, alias_fqn, &ir)?;
    Ok(ir)
}

fn assert_typealias_rhs_is_public(
    index: &Index,
    alias_fqn: &str,
    ty: &IrType,
) -> Result<(), ScoopIrExportError> {
    match ty {
        IrType::Named { fqn, args, eff } => {
            let Some(syms) = index.by_fqn.get(fqn) else {
                return Err(ScoopIrExportError::TypeAliasReferencesUnknownType {
                    alias_fqn: alias_fqn.to_string(),
                    referenced_fqn: fqn.clone(),
                });
            };
            let Some(sym) = syms.ty.as_ref() else {
                return Err(ScoopIrExportError::TypeAliasReferencesUnknownType {
                    alias_fqn: alias_fqn.to_string(),
                    referenced_fqn: fqn.clone(),
                });
            };
            if sym.visibility != Visibility::Public {
                return Err(ScoopIrExportError::TypeAliasExposesNonPublicType {
                    alias_fqn: alias_fqn.to_string(),
                    referenced_fqn: fqn.clone(),
                });
            }
            for a in args {
                assert_typealias_rhs_is_public(index, alias_fqn, a)?;
            }
            if let Some(row) = eff {
                for t in &row.terms {
                    assert_typealias_rhs_is_public(index, alias_fqn, t)?;
                }
            }
            Ok(())
        }
        IrType::Param { .. } => Ok(()),
        IrType::Tuple { elements } => {
            for e in elements {
                assert_typealias_rhs_is_public(index, alias_fqn, e)?;
            }
            Ok(())
        }
        IrType::Function {
            receiver,
            params,
            return_ty,
            effects,
        } => {
            if let Some(r) = receiver.as_ref() {
                assert_typealias_rhs_is_public(index, alias_fqn, r)?;
            }
            for p in params {
                assert_typealias_rhs_is_public(index, alias_fqn, p)?;
            }
            assert_typealias_rhs_is_public(index, alias_fqn, return_ty)?;
            for t in &effects.terms {
                assert_typealias_rhs_is_public(index, alias_fqn, t)?;
            }
            Ok(())
        }
        IrType::Union { variants } => {
            for v in variants {
                assert_typealias_rhs_is_public(index, alias_fqn, v)?;
            }
            Ok(())
        }
    }
}

/// 收集某个源文件内声明的 `public` 函数（fun header）。
///
/// v0 策略：
/// - 以 HIR 的签名类型（`TypeId`）为准生成 `IrType`；
/// - 以 Index 的可见性过滤 public（避免把 internal/private 导出到 `.cone`）。
fn export_public_funs_for_source(
    source: &SourceFile,
    index: &Index,
    hir: &LoweredHir,
) -> Result<Vec<IrFunDecl>, ScoopIrExportError> {
    let mut out: Vec<IrFunDecl> = Vec::new();

    for item in &hir.file.items {
        let HirItem::Fun(fun) = item else {
            continue;
        };

        // v0：用 Index 的可见性作为 “public API” 的过滤条件。
        let Some(ns) = index.by_fqn.get(&fun.fqn) else {
            return Err(ScoopIrExportError::MissingFunSymbol {
                fqn: fun.fqn.clone(),
            });
        };
        let Some(overload) = ns.fun.iter().find(|o| {
            o.symbol.decl_file.as_path() == source.path()
                && o.symbol.visibility == Visibility::Public
        }) else {
            continue;
        };

        let kind = match overload.sig.kind {
            crate::ast::FunDeclKind::Regular => IrFunDeclKind::Regular,
            crate::ast::FunDeclKind::EffectOp => IrFunDeclKind::EffectOp,
        };

        let FunctionType {
            receiver, effects, ..
        } = decode_function_type(&hir.types, fun.ty, &fun.fqn)?;

        let receiver_ir = receiver.map(|id| to_ir_type(&hir.types, id, 0));
        let params = fun
            .params
            .iter()
            .map(|p| IrFunParam {
                name: p.name.clone(),
                ty: to_ir_type(&hir.types, p.ty, 0),
            })
            .collect::<Vec<_>>();

        let return_ty = to_ir_type(&hir.types, fun.return_ty, 0);
        let effects = to_ir_effect_row(&hir.types, &effects, 0);

        let type_params =
            collect_type_params_for_fun(&hir.types, receiver, &params, &return_ty, &effects);

        out.push(IrFunDecl {
            fqn: fun.fqn.clone(),
            kind,
            type_params,
            receiver: receiver_ir,
            params,
            return_ty,
            effects,
        });
    }

    Ok(out)
}

/// 从 `TypeStore` 中解码一个函数类型。
fn decode_function_type(
    store: &TypeStore,
    fun_ty: TypeId,
    fqn: &str,
) -> Result<FunctionType, ScoopIrExportError> {
    match store.kind(fun_ty) {
        TypeKind::Ref(RefTypeKind::Function(ft)) => Ok(ft.clone()),
        _ => Err(ScoopIrExportError::UnexpectedFunType {
            fqn: fqn.to_string(),
        }),
    }
}

/// 从函数签名里提取“被引用到的 type param 名字”。
///
/// 注意：v0 不保证包含所有“声明处但未使用”的 type params；其完善策略见后续任务（T1106+）。
fn collect_type_params_for_fun(
    store: &TypeStore,
    receiver: Option<TypeId>,
    params: &[IrFunParam],
    return_ty: &IrType,
    effects: &IrEffectRow,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if let Some(r) = receiver {
        collect_type_params_from_type_id(store, r, &mut seen, &mut out, 0);
    }
    for p in params {
        collect_type_params_from_ir_type(&p.ty, &mut seen, &mut out);
    }
    collect_type_params_from_ir_type(return_ty, &mut seen, &mut out);
    for term in &effects.terms {
        collect_type_params_from_ir_type(term, &mut seen, &mut out);
    }

    out
}

/// 递归扫描 `TypeId` 并把遇到的 `TypeKind::Param` 名字写入 `out`（去重且保序）。
fn collect_type_params_from_type_id(
    store: &TypeStore,
    id: TypeId,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }

    match store.kind(id) {
        TypeKind::Param(p) => {
            if seen.insert(p.name.clone()) {
                out.push(p.name.clone());
            }
        }
        TypeKind::Ref(RefTypeKind::Nominal(NominalType { args, eff, .. }))
        | TypeKind::Value(ValueTypeKind::Nominal(NominalType { args, eff, .. })) => {
            for a in args {
                collect_type_params_from_type_id(store, *a, seen, out, depth + 1);
            }
            if let Some(row) = eff {
                for t in &row.terms {
                    collect_type_params_from_type_id(store, *t, seen, out, depth + 1);
                }
            }
        }
        TypeKind::Ref(RefTypeKind::Function(FunctionType {
            receiver,
            params,
            return_ty,
            effects,
            ..
        })) => {
            if let Some(r) = receiver {
                collect_type_params_from_type_id(store, *r, seen, out, depth + 1);
            }
            for p in params {
                collect_type_params_from_type_id(store, *p, seen, out, depth + 1);
            }
            collect_type_params_from_type_id(store, *return_ty, seen, out, depth + 1);
            for t in &effects.terms {
                collect_type_params_from_type_id(store, *t, seen, out, depth + 1);
            }
        }
        TypeKind::Ref(RefTypeKind::Union(UnionType { variants })) => {
            for v in variants {
                collect_type_params_from_type_id(store, *v, seen, out, depth + 1);
            }
        }
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            collect_type_params_from_type_id(store, *inner, seen, out, depth + 1);
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            for e in elements {
                collect_type_params_from_type_id(store, *e, seen, out, depth + 1);
            }
        }
        TypeKind::Ref(RefTypeKind::Any)
        | TypeKind::Ref(RefTypeKind::String)
        | TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
    }
}

/// 递归扫描 IR 类型表达式，补齐 `type_params` 列表（去重且保序）。
fn collect_type_params_from_ir_type(
    ty: &IrType,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    match ty {
        IrType::Named { args, eff, .. } => {
            for a in args {
                collect_type_params_from_ir_type(a, seen, out);
            }
            if let Some(row) = eff {
                for t in &row.terms {
                    collect_type_params_from_ir_type(t, seen, out);
                }
            }
        }
        IrType::Param { name } => {
            if seen.insert(name.clone()) {
                out.push(name.clone());
            }
        }
        IrType::Tuple { elements } => {
            for e in elements {
                collect_type_params_from_ir_type(e, seen, out);
            }
        }
        IrType::Function {
            receiver,
            params,
            return_ty,
            effects,
        } => {
            if let Some(r) = receiver {
                collect_type_params_from_ir_type(r, seen, out);
            }
            for p in params {
                collect_type_params_from_ir_type(p, seen, out);
            }
            collect_type_params_from_ir_type(return_ty, seen, out);
            for t in &effects.terms {
                collect_type_params_from_ir_type(t, seen, out);
            }
        }
        IrType::Union { variants } => {
            for v in variants {
                collect_type_params_from_ir_type(v, seen, out);
            }
        }
    }
}

/// 转换 effect row（v0：仅保留 term 集合）。
fn to_ir_effect_row(store: &TypeStore, row: &EffectRow, depth: usize) -> IrEffectRow {
    IrEffectRow {
        terms: row
            .terms
            .iter()
            .copied()
            .map(|id| to_ir_type(store, id, depth + 1))
            .collect(),
    }
}

/// 转换编译器内部 `TypeId` 到稳定的 ScoopIR 类型表达式（v0）。
fn to_ir_type(store: &TypeStore, id: TypeId, depth: usize) -> IrType {
    // 防御性：避免未来引入递归类型时的栈爆。
    if depth > 64 {
        return IrType::Named {
            fqn: "<type-recursion>".to_string(),
            args: Vec::new(),
            eff: None,
        };
    }

    match store.kind(id) {
        TypeKind::Ref(RefTypeKind::Any) => IrType::Named {
            fqn: "scoop.core.Any".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Ref(RefTypeKind::String) => IrType::Named {
            fqn: "scoop.core.String".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Ref(RefTypeKind::Nominal(n)) => ir_nominal(store, n, depth),
        TypeKind::Ref(RefTypeKind::Function(fun)) => IrType::Function {
            receiver: fun
                .receiver
                .map(|r| Box::new(to_ir_type(store, r, depth + 1))),
            params: fun
                .params
                .iter()
                .copied()
                .map(|p| to_ir_type(store, p, depth + 1))
                .collect(),
            return_ty: Box::new(to_ir_type(store, fun.return_ty, depth + 1)),
            effects: to_ir_effect_row(store, &fun.effects, depth + 1),
        },
        TypeKind::Ref(RefTypeKind::Union(u)) => IrType::Union {
            variants: u
                .variants
                .iter()
                .copied()
                .map(|v| to_ir_type(store, v, depth + 1))
                .collect(),
        },
        TypeKind::Value(ValueTypeKind::Unit) => IrType::Named {
            fqn: "scoop.core.Unit".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::Nothing) => IrType::Named {
            fqn: "scoop.core.Nothing".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::Bool) => IrType::Named {
            fqn: "scoop.core.Bool".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::Int) => IrType::Named {
            fqn: "scoop.core.Int".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::UInt) => IrType::Named {
            fqn: "scoop.core.UInt".to_string(),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::IntN(bits)) => IrType::Named {
            fqn: format!("scoop.core.Int{bits}"),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => IrType::Named {
            fqn: format!("scoop.core.UInt{bits}"),
            args: Vec::new(),
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::Option(inner)) => IrType::Named {
            fqn: "scoop.core.Option".to_string(),
            args: vec![to_ir_type(store, *inner, depth + 1)],
            eff: None,
        },
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => IrType::Tuple {
            elements: elements
                .iter()
                .copied()
                .map(|e| to_ir_type(store, e, depth + 1))
                .collect(),
        },
        TypeKind::Value(ValueTypeKind::Nominal(n)) => ir_nominal(store, n, depth),
        TypeKind::Param(p) => IrType::Param {
            name: p.name.clone(),
        },
    }
}

/// 转换名义类型（nominal type）：`FQN + type args (+ eff row arg)`。
fn ir_nominal(store: &TypeStore, n: &NominalType, depth: usize) -> IrType {
    IrType::Named {
        fqn: n.fqn.clone(),
        args: n
            .args
            .iter()
            .copied()
            .map(|a| to_ir_type(store, a, depth + 1))
            .collect(),
        eff: n
            .eff
            .as_ref()
            .map(|row| to_ir_effect_row(store, row, depth + 1)),
    }
}
