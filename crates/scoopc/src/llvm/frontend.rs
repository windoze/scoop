use std::path::{Path, PathBuf};

use crate::ast;
use crate::hir;
use crate::opt::OptLevel;
use crate::resolve::Index;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::ty::TypeStore;
use crate::typecheck::TypeEnv;

use super::LlvmEmitError;

pub(super) struct SingleFileCodegenUnit {
    pub(super) lowered: hir::LoweredHir,
    pub(super) source_map: SourceMap,
    pub(super) entry_source_id: SourceId,
}

/// 为 `emit_minimal_main_ir` / `emit_minimal_main_obj_to_file` 准备“单文件 + support sources”的
/// 完整 codegen 输入。
///
/// 约定：
/// - `session.sysroot().files` 继续只提供签名型 sysroot；
/// - `stdlib/*.scoop` 与 `session.sysroot().compilable_source_paths` 中的纯 Scoop 实现文件会与当前
///   输入文件一起进入 resolve/typecheck/lowering；
/// - 但只有入口源文件本身允许贡献 monomorphization 请求种子；support sources 只作为可被调用的
///   实现体参与 lowering / codegen，避免把它们内部未被入口触达的 generic 调用提升为实例根；
/// - 这样 single-file LLVM 路径就能复用 build 管线需要的 typecheck side table 与多文件 lowering，
///   同时不再依赖 `lower_for_dump` 的最小调试路径；
/// - 返回的 `lowered` 仍承载当前 LLVM codegen 需要的 HIR 兼容输入，但会保留
///   `LoweredHir::materialized_pass_view()`，作为 production 主路径显式接入的 canonical
///   materialized body / summary / 后续 MIR pass 产物视图。
#[cfg(test)]
pub(super) fn prepare_single_file_codegen_unit(
    session: &Session,
    entry_source: &SourceFile,
) -> Result<SingleFileCodegenUnit, LlvmEmitError> {
    prepare_single_file_codegen_unit_with_opt_level(session, entry_source, OptLevel::O0)
}

pub(super) fn prepare_single_file_codegen_unit_with_opt_level(
    session: &Session,
    entry_source: &SourceFile,
    opt_level: OptLevel,
) -> Result<SingleFileCodegenUnit, LlvmEmitError> {
    let mut input_sources = load_single_file_support_sources(session)?;
    let entry_index = input_sources.len();
    input_sources.push(entry_source.clone());

    let mut asts = Vec::with_capacity(input_sources.len());
    for source in &input_sources {
        let ast =
            crate::effect_refactor_pipeline::enter_ast_stage(session, || session.parse(source))
                .map_err(frontend_error)?;
        asts.push(ast);
    }
    {
        let source_refs = input_sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(frontend_error)?;
    }
    for (source, ast) in input_sources.iter().zip(asts.iter()) {
        crate::typecheck::check_file_headers(source, ast).map_err(frontend_error)?;
        crate::typecheck::check_file_struct_decls(source, ast).map_err(frontend_error)?;
    }

    let index = build_single_file_index(session, &input_sources, &asts).map_err(frontend_error)?;

    let mut headers = Vec::with_capacity(input_sources.len());
    for (source, ast) in input_sources.iter().zip(asts.iter()) {
        let header =
            crate::resolve::check_file_headers(source, ast, &index).map_err(frontend_error)?;
        headers.push(header);
    }
    for ((source, ast), header) in input_sources
        .iter()
        .zip(asts.iter_mut())
        .zip(headers.iter())
    {
        crate::resolve::check_file_bodies(source, ast, &index, header).map_err(frontend_error)?;
    }

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).map_err(frontend_error)?;
    for (source, ast) in input_sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(frontend_error)?;
    }

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    let mut monomorph_requests = Vec::new();

    for (source_index, ((source, ast), header)) in input_sources
        .iter()
        .zip(asts.iter())
        .zip(headers.iter())
        .enumerate()
    {
        crate::typecheck::check_file_annotations(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .map_err(frontend_error)?;
        crate::typecheck::check_file_properties(source, ast, &index, &env)
            .map_err(|err| frontend_error(*err))?;
        crate::typecheck::check_file_inheritance(source, ast, &index).map_err(frontend_error)?;
        crate::typecheck::check_file_interfaces(source, ast, &index, &env)
            .map_err(frontend_error)?;
        crate::typecheck::check_file_override_effects(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .map_err(|err| frontend_error(*err))?;
        crate::typecheck::check_file_type_refs(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .map_err(frontend_error)?;
        crate::typecheck::check_file_where_clauses(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .map_err(frontend_error)?;
        crate::typecheck::check_file_overload_conflicts(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .map_err(frontend_error)?;

        if source_index == entry_index {
            let requests = crate::typecheck::check_file_exprs_with_monomorph_requests(
                source,
                ast,
                &index,
                &header.imports,
                &env,
                &mut typecheck_types,
                builtins,
            )
            .map_err(frontend_error)?;
            monomorph_requests.extend(requests);
        } else {
            crate::typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &header.imports,
                &env,
                &mut typecheck_types,
                builtins,
            )
            .map_err(frontend_error)?;
        }
    }

    crate::typecheck::check_file_type_layouts(&index, &env, &mut typecheck_types, builtins)
        .map_err(frontend_error)?;

    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> =
        Vec::with_capacity(session.sysroot().files.len() + input_sources.len());
    for file in &session.sysroot().files {
        compilation_unit.push((&file.source, &file.ast));
    }
    for (source, ast) in input_sources.iter().zip(asts.iter()) {
        compilation_unit.push((source, ast));
    }

    let files_to_lower = input_sources
        .iter()
        .zip(asts.iter())
        .collect::<Vec<(&SourceFile, &ast::File)>>();
    let request_source_paths = vec![entry_source.path().to_path_buf()];
    let lowered =
        hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        &index,
        &compilation_unit,
        &files_to_lower,
        &monomorph_requests,
        Some(&env),
        &typecheck_types,
        hir::MirInstanceCollectionOptions {
            request_source_paths: &request_source_paths,
            request_root_mode: crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
            opt_level,
        },
    )
    .map_err(|err| frontend_error(err.to_string()))?;

    let (source_map, entry_source_id) =
        build_source_map_with_extra_sources(session, &input_sources, entry_index);

    Ok(SingleFileCodegenUnit {
        lowered,
        source_map,
        entry_source_id,
    })
}

fn load_single_file_support_sources(session: &Session) -> Result<Vec<SourceFile>, LlvmEmitError> {
    let stdlib_root = default_stdlib_path();
    let stdlib_root = stdlib_root.canonicalize().map_err(|error| {
        frontend_error(format!(
            "无法定位 stdlib 目录：{}: {error}",
            stdlib_root.display()
        ))
    })?;

    let mut paths = Vec::new();
    collect_scoop_files(&stdlib_root, &mut paths)?;
    paths.extend(session.sysroot().compilable_source_paths.iter().cloned());
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        sources.push(SourceFile::load(&path).map_err(frontend_error)?);
    }
    Ok(sources)
}

fn build_single_file_index(
    session: &Session,
    input_sources: &[SourceFile],
    asts: &[ast::File],
) -> Result<Index, crate::resolve::ResolveError> {
    let mut pairs: Vec<(&SourceFile, &ast::File)> =
        Vec::with_capacity(session.sysroot().files.len() + input_sources.len());
    for file in &session.sysroot().files {
        pairs.push((&file.source, &file.ast));
    }
    for (source, ast) in input_sources.iter().zip(asts.iter()) {
        pairs.push((source, ast));
    }
    Index::build(&pairs)
}

fn frontend_error(error: impl std::fmt::Display) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: error.to_string(),
    }
}

fn default_stdlib_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LlvmEmitError> {
    for entry in std::fs::read_dir(dir)
        .map_err(|error| frontend_error(format!("无法读取目录：{}: {error}", dir.display())))?
    {
        let entry = entry.map_err(frontend_error)?;
        let path = entry.path();
        let ty = entry.file_type().map_err(frontend_error)?;
        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

pub(super) fn build_source_map_with_extra_sources(
    session: &Session,
    input_sources: &[SourceFile],
    entry_index: usize,
) -> (SourceMap, SourceId) {
    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }

    let mut entry_source_id = None;
    for (idx, source) in input_sources.iter().enumerate() {
        let source_id = source_map.add_source_clone(source);
        if idx == entry_index {
            entry_source_id = Some(source_id);
        }
    }

    (
        source_map,
        entry_source_id.expect("entry source should always be present in source map"),
    )
}
