//! `scoop build` 子命令。
//!
//! T0805：实现“前端检查 + 输出路径准备”。
//!
//! T0806：在启用 `scoop` 的 `llvm` feature 时，额外执行：
//! - 生成最小 object（当前阶段仍是固定 `main → ret 0`）；
//! - 调用 clang 链接 object + 早期 C runtime，产出可执行文件。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 执行 `scoop build <input> [-o <output>]`。
///
/// 当前阶段验收点：
/// - 输入可通过 parse/resolve/typecheck 时返回 `Ok(())`；
/// - `output` 仅用于“准备输出路径”，不会实际写入二进制文件。
pub fn run(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = output.unwrap_or_else(default_output_path);
    ensure_output_parent_dir(&output)?;

    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    let source = scoopc::source::SourceFile::load(&input)?;
    let session = scoopc::session::Session::new()?;

    run_frontend(&session, &source)?;

    // 只有在启用 LLVM 后端时才会真正生成可执行文件；默认构建仍保持“前端检查”可用。
    #[cfg(feature = "llvm")]
    run_codegen_and_link(&session, &source, &output)?;

    Ok(())
}

fn run_frontend(session: &scoopc::session::Session, source: &scoopc::source::SourceFile) -> Result<()> {
    let mut ast = scoopc::parser::parse_file(source).map_err(miette::Report::from)?;

    // 先运行不依赖 resolver/index 的 typecheck 预检查（与 fixtures/typecheck pipeline 对齐）。
    scoopc::typecheck::check_file_headers(source, &ast).map_err(miette::Report::from)?;
    scoopc::typecheck::check_file_struct_decls(source, &ast).map_err(miette::Report::from)?;

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(miette::Report::from)?;

    let headers = scoopc::resolve::check_file_headers(source, &ast, &index).map_err(miette::Report::from)?;
    scoopc::resolve::check_file_bodies(source, &mut ast, &index, &headers).map_err(miette::Report::from)?;

    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    env.extend_from_file(source, &ast, &index)
        .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_properties(source, &ast, &index, &env).map_err(miette::Report::from)?;
    scoopc::typecheck::check_file_inheritance(source, &ast, &index).map_err(miette::Report::from)?;

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    scoopc::typecheck::check_file_interfaces(source, &ast, &index, &env).map_err(miette::Report::from)?;
    scoopc::typecheck::check_file_override_effects(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_where_clauses(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_overload_conflicts(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(miette::Report::from)?;

    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;

    Ok(())
}

fn ensure_output_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .into_diagnostic()
        .wrap_err("无法创建输出目录")?;
    Ok(())
}

fn default_output_path() -> PathBuf {
    let ext = std::env::consts::EXE_EXTENSION;
    if ext.is_empty() {
        PathBuf::from("a.out")
    } else {
        PathBuf::from(format!("a.{ext}"))
    }
}

#[cfg(feature = "llvm")]
fn run_codegen_and_link(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    output: &Path,
) -> Result<()> {
    let dir = super::temp::make_temp_dir("scoop_build")?;
    let obj = dir.join("main.o");

    scoopc::llvm::emit_minimal_main_obj_to_file(session, source, &obj)?;
    crate::toolchain::link_obj_with_runtime(&obj, output)?;

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    #[test]
    fn build_frontend_ok_and_creates_parent_dir() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("nested").join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(input, Some(out)).unwrap();
        assert!(dir.path().join("nested").is_dir());
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn build_produces_executable_and_it_runs() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(input, Some(out.clone())).unwrap();
        assert!(out.is_file(), "build 应写出可执行文件");

        let status = std::process::Command::new(&out).status().unwrap();
        assert!(status.success(), "可执行文件应返回 0");
    }
}
