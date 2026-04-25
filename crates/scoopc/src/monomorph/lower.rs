//! 单态化（monomorphization）实例生成：从 typecheck 收集的 `MonomorphKey` 出发，
//! 生成“具体实例”的 MIR（T0712）。
//!
//! 当前阶段目标（最小可回归落点）：
//! - `lower_for_dump` 仍按“单文件输入 + sysroot”建模，因此这里只对“当前文件内”的泛型函数调用生成实例；
//! - 只实例化 type params（`fun <T>`）；effect row 参数与名义类型泛型后置；
//! - 生成的实例以 `fqn::<TypeArgs...>` 命名，并做去重缓存（同 key 只生成一次）。
//!
//! 说明：
//! - build/run 的 compilation-unit 多文件主线不经过本模块，而是由 `scoop build/run` 收集全部源文件的
//!   monomorph keys，并在 HIR lowering 的 `collect_generic_fun_instantiations` 中完成跨文件实例化。

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
    Comptime(#[from] crate::comptime::ConstEvalError),

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

    #[error(transparent)]
    #[diagnostic(transparent)]
    VtableLayout(#[from] crate::vtable::VtableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ItableLayout(#[from] crate::itable::ItableLayoutError),

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

type MonomorphLowerResult<T> = Result<T, Box<MonomorphLowerError>>;

fn monomorph_lower_err(error: MonomorphLowerError) -> Box<MonomorphLowerError> {
    Box::new(error)
}

impl From<ParseError> for Box<MonomorphLowerError> {
    fn from(error: ParseError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<crate::comptime::ConstEvalError> for Box<MonomorphLowerError> {
    fn from(error: crate::comptime::ConstEvalError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<ResolveError> for Box<MonomorphLowerError> {
    fn from(error: ResolveError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<TypeHeaderError> for Box<MonomorphLowerError> {
    fn from(error: TypeHeaderError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<StructDeclError> for Box<MonomorphLowerError> {
    fn from(error: StructDeclError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<TypeEnvError> for Box<MonomorphLowerError> {
    fn from(error: TypeEnvError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<TypeLowerError> for Box<MonomorphLowerError> {
    fn from(error: TypeLowerError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<AnnotationError> for Box<MonomorphLowerError> {
    fn from(error: AnnotationError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<ExprTypeError> for Box<MonomorphLowerError> {
    fn from(error: ExprTypeError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<crate::vtable::VtableLayoutError> for Box<MonomorphLowerError> {
    fn from(error: crate::vtable::VtableLayoutError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
}

impl From<crate::itable::ItableLayoutError> for Box<MonomorphLowerError> {
    fn from(error: crate::itable::ItableLayoutError) -> Self {
        monomorph_lower_err(MonomorphLowerError::from(error))
    }
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
) -> MonomorphLowerResult<LoweredMonomorphMir> {
    // 1) parse + headers 预检查（不依赖 resolver/index）
    let mut file = parse_file(source)?;
    {
        let sources = [source];
        let mut files = [&mut file];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }
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
    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for sysroot_file in &session.sysroot().files {
        compilation_unit.push((&sysroot_file.source, &sysroot_file.ast));
    }
    compilation_unit.push((source, &file));
    let class_vtables = crate::vtable::collect_class_vtables(&compilation_unit, &index)?;
    let (interfaces, _class_itables) = crate::itable::collect_interfaces_and_class_itables(
        &compilation_unit,
        &index,
        &class_vtables,
    )?;
    let mir_facts = crate::mir::MirLoweringFacts::from_dispatch_tables_and_resume_spans(
        &class_vtables,
        &interfaces,
        file.continuation_resume_call_sites(),
        file.non_pure_continuation_resume_call_sites(),
    );

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

        // `dump-ir` 仍是单文件调试入口：这里只实例化当前输入文件内的顶层函数。
        if key.symbol.decl_file != source.path() {
            continue;
        }

        let Some(fun_decl) = fun_index.get(&(key.symbol.fqn.clone(), key.symbol.decl_span)) else {
            return Err(monomorph_lower_err(
                MonomorphLowerError::MissingFunDeclForInstance {
                    fqn: key.symbol.fqn.clone(),
                    file: key.symbol.decl_file.display().to_string(),
                    span: key.symbol.decl_span,
                },
            ));
        };

        if fun_decl.type_params.len() != key.type_args.len() {
            return Err(monomorph_lower_err(
                MonomorphLowerError::TypeArgArityMismatchForInstance {
                    fqn: key.symbol.fqn.clone(),
                    expected: fun_decl.type_params.len(),
                    found: key.type_args.len(),
                    decl_span: fun_decl.name.span.into(),
                },
            ));
        }

        let mut bindings: Vec<(String, TypeId)> = Vec::with_capacity(fun_decl.type_params.len());
        for (idx, p) in fun_decl.type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            bindings.push((name, key.type_args[idx]));
        }

        // 先降低到 HIR（type params 已绑定到具体类型），再走 MIR lowering。
        let mut hir_fun = crate::hir::lower_fun_with_type_bindings(
            crate::hir::LoweringInputs {
                source,
                file: &file,
                index: &index,
                type_kinds: &type_kinds,
                typecheck_types: None,
                types: &mut types,
                builtins,
            },
            fun_decl,
            bindings,
        );

        let instance_fqn = monomorph_instance_fqn(&key.symbol.fqn, &key.type_args, &types);
        hir_fun.fqn = instance_fqn.clone();

        let hir_file = crate::hir::File {
            items: vec![crate::hir::Item::Fun(hir_fun)],
        };
        let mir_file = crate::mir::lower_hir_file_for_dump_with_facts(
            builtins, &mut types, &hir_file, &mir_facts,
        );
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

    #[test]
    fn monomorph_preserves_virtual_call_kind_in_instantiated_body() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_virtual_call.scoop",
            r#"
package fixtures.monomorph

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

fun use<T>(marker: T, b: Base): Int {
    return b.ping()
}

fun entry(b: Base): Int {
    return use(1, b)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn.contains("use::<Int>") => Some(fun),
                _ => None,
            })
            .expect("expected monomorphized use::<Int> instance");
        let body = fun
            .body
            .as_ref()
            .expect("monomorphized instance should have body");
        let stmt = body.blocks[0]
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .expect("expected call in monomorphized body");

        match stmt {
            crate::mir::CallKind::Virtual { dispatch, .. } => {
                assert_eq!(dispatch.owner_fqn, "fixtures.monomorph.Base");
                assert_eq!(dispatch.member_name, "ping");
            }
            other => panic!("expected virtual call kind, got {other:?}"),
        }
    }
}
