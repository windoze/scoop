//! 单态化（monomorphization）实例生成：从 typecheck 收集的 `MonomorphKey` 出发，
//! 生成“具体实例”的 MIR（T0712）。
//!
//! 当前阶段目标（最小可回归落点）：
//! - 只对“当前文件内”的泛型函数调用生成实例；
//! - 只实例化 type params（`fun <T>`）；effect row 参数与名义类型泛型后置；
//! - 生成的实例以 `fqn::<TypeArgs...>` 命名，并做去重缓存（同 key 只生成一次）。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{TypeId, TypeStore};
use crate::typecheck;
use crate::typecheck::{
    AnnotationError, ExprTypeError, StructDeclError, TypeEnv, TypeEnvError, TypeHeaderError,
    TypeLowerError,
};

use super::MonomorphKey;

/// 单态化实例生成（monomorphization）的 lowering 错误。
#[derive(Debug, Error, Diagnostic)]
pub enum MonomorphLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeHeader(#[from] TypeHeaderError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    StructDecl(#[from] StructDeclError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeEnv(#[from] TypeEnvError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Annotation(#[from] AnnotationError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ExprType(#[from] ExprTypeError),

    #[error("单态化实例生成缺少函数声明：{fqn}@{file}:{span:?}")]
    #[diagnostic(code(scoop::monomorph::missing_fun_decl_for_instance))]
    MissingFunDeclForInstance {
        fqn: String,
        file: String,
        span: Span,
    },

    #[error("单态化实例生成的 type args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::monomorph::type_arg_arity_mismatch_for_instance))]
    TypeArgArityMismatchForInstance {
        fqn: String,
        expected: usize,
        found: usize,
        #[label("函数声明在这里")]
        decl_span: miette::SourceSpan,
    },
}

/// 单态化后的 MIR：包含“被调用到的泛型函数实例”的 MIR 视图。
///
/// 注意：`file` 内的 `TypeId` 需要配合 `types` 才能做进一步解释（例如打印为 `Int/String`）。
#[derive(Debug)]
pub struct LoweredMonomorphMir {
    pub file: crate::mir::File,
    pub types: TypeStore,
    pub keys: Vec<MonomorphKey>,
}

/// 生成“单态化实例 MIR”（用于 `scoop dump-ir` / unit tests）。
pub fn lower_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<LoweredMonomorphMir, MonomorphLowerError> {
    // 1) parse + headers 预检查（不依赖 resolver/index）
    let mut file = parse_file(source)?;
    typecheck::check_file_headers(source, &file)?;
    typecheck::check_file_struct_decls(source, &file)?;

    // 2) build index（sysroot + 当前文件）
    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((source, &file));
        Index::build(&pairs)?
    };

    // 3) resolver：headers + bodies（写回 ValueIdent.resolved 等信息）
    let resolved_headers = crate::resolve::check_file_headers(source, &file, &index)?;
    crate::resolve::check_file_bodies(source, &mut file, &index, &resolved_headers)?;

    // 4) type env：sysroot + 当前文件
    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
    env.extend_from_file(source, &file, &index)?;

    // 5) typecheck expr，并收集 monomorph keys（T0712）
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    typecheck::check_file_annotations(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;

    typecheck::check_file_type_refs(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;

    let keys = typecheck::check_file_exprs_with_monomorph_keys(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;

    // 6) 生成实例 MIR（同 key 只生成一次）。
    let fun_index = index_file_fun_decls(source, &file);
    let type_kinds = collect_type_decl_kinds(session, source, &file);

    let mut seen: HashSet<MonomorphKey> = HashSet::new();
    let mut instances: Vec<(String, crate::mir::File)> = Vec::new();

    // 为保证 dump 输出稳定：按“实例名”排序后再生成。
    let mut keys_sorted = keys.clone();
    keys_sorted.sort_by(|a, b| {
        monomorph_instance_fqn(&a.symbol.fqn, &a.type_args, &types).cmp(&monomorph_instance_fqn(
            &b.symbol.fqn,
            &b.type_args,
            &types,
        ))
    });

    for key in &keys_sorted {
        if !seen.insert(key.clone()) {
            continue;
        }

        // 当前阶段：仅支持实例化“当前文件内”的顶层函数。
        if key.symbol.decl_file != source.path() {
            continue;
        }

        let Some(fun_decl) = fun_index.get(&(key.symbol.fqn.clone(), key.symbol.decl_span)) else {
            return Err(MonomorphLowerError::MissingFunDeclForInstance {
                fqn: key.symbol.fqn.clone(),
                file: key.symbol.decl_file.display().to_string(),
                span: key.symbol.decl_span,
            });
        };

        if fun_decl.type_params.len() != key.type_args.len() {
            return Err(MonomorphLowerError::TypeArgArityMismatchForInstance {
                fqn: key.symbol.fqn.clone(),
                expected: fun_decl.type_params.len(),
                found: key.type_args.len(),
                decl_span: fun_decl.name.span.into(),
            });
        }

        let mut bindings: Vec<(String, TypeId)> = Vec::with_capacity(fun_decl.type_params.len());
        for (idx, p) in fun_decl.type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            bindings.push((name, key.type_args[idx]));
        }

        // 先降低到 HIR（type params 已绑定到具体类型），再走 MIR lowering。
        let mut hir_fun = crate::hir::lower_fun_with_type_bindings(
            source,
            &file,
            &index,
            &type_kinds,
            &mut types,
            builtins,
            fun_decl,
            bindings,
        );

        let instance_fqn = monomorph_instance_fqn(&key.symbol.fqn, &key.type_args, &types);
        hir_fun.fqn = instance_fqn.clone();

        let hir_file = crate::hir::File {
            items: vec![crate::hir::Item::Fun(hir_fun)],
        };
        let mir_file = crate::mir::lower_hir_file_for_dump(builtins, &mut types, &hir_file);
        instances.push((instance_fqn, mir_file));
    }

    // 合并所有实例的 items 为一份 MIR 文件（便于 dump）。
    instances.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut out_items = Vec::new();
    for (_, f) in instances {
        out_items.extend(f.items);
    }

    Ok(LoweredMonomorphMir {
        file: crate::mir::File { items: out_items },
        types,
        keys,
    })
}

fn monomorph_instance_fqn(base: &str, type_args: &[TypeId], types: &TypeStore) -> String {
    if type_args.is_empty() {
        return base.to_string();
    }
    let args = type_args
        .iter()
        .copied()
        .map(|id| types.display(id).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base}::<{args}>")
}

fn index_file_fun_decls<'a>(
    source: &SourceFile,
    file: &'a ast::File,
) -> HashMap<(String, Span), &'a ast::FunDecl> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out: HashMap<(String, Span), &'a ast::FunDecl> = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let local_name = source.slice(fun.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        out.insert((fqn, fun.name.span), fun);
    }

    out
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(p) = package else {
        return String::new();
    };
    p.path
        .iter()
        .map(|seg| seg.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn collect_type_decl_kinds(
    session: &Session,
    source: &SourceFile,
    file: &ast::File,
) -> HashMap<String, ast::TypeKind> {
    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, file));

    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monomorph_collects_two_instances_for_id() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_id.scoop",
            r#"
package fixtures.monomorph

import scoop.core.*

fun id<T>(x: T): T {
    return x
}

fun f() {
    val a = id(1)
    val b = id("s")
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fqn_list: Vec<String> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::mir::Item::Fun(fun) => Some(fun.fqn.clone()),
                _ => None,
            })
            .collect();

        assert!(fqn_list.iter().any(|fqn| fqn.contains("id::<Int>")));
        assert!(fqn_list.iter().any(|fqn| fqn.contains("id::<String>")));
        assert_eq!(
            fqn_list.iter().filter(|fqn| fqn.contains("id::<")).count(),
            2
        );
    }
}
