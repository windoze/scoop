//! Fixtures 运行器（`scoop test`）。
//!
//! 设计目标：
//! - fixtures 是 Scoop 实现正确性的“底座”，必须可长期维护
//! - 每个 `.scoop` 文件可通过注释声明期望（pass/fail、错误包含等）
//!
//! 当前阶段支持：
//! - parse fixtures（调用 `scoopc::parser::parse_file`）
//! - resolve fixtures（最小名字绑定：import + TypeRef 解析）

mod expectations;

use std::path::{Path, PathBuf};
use std::path::Component;

use miette::{miette, Context as _, IntoDiagnostic as _, Result};

use expectations::{Expect, FixtureExpectation};

pub fn run_all(fixtures_root: &Path) -> Result<usize> {
    let mut files = Vec::new();
    collect_scoop_files(fixtures_root, &mut files)?;

    if files.is_empty() {
        return Err(miette!(
            "fixtures 目录下未发现任何 .scoop 文件：{}",
            fixtures_root.display()
        ));
    }

    let mut ok = 0usize;
    let session = scoopc::session::Session::new()?;
    for file in files {
        run_one(&session, fixtures_root, &file)
            .wrap_err_with(|| format!("fixture 失败：{}", file.display()))?;
        ok += 1;
    }

    Ok(ok)
}

fn run_one(session: &scoopc::session::Session, fixtures_root: &Path, path: &Path) -> Result<()> {
    let source = scoopc::source::SourceFile::load(path)?;
    let exp = FixtureExpectation::from_source(source.text());

    let rel = path.strip_prefix(fixtures_root).unwrap_or(path);
    let phase = match rel.components().next() {
        Some(Component::Normal(name)) if name == "resolve" => FixturePhase::Resolve,
        _ => FixturePhase::Parse,
    };

    let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = match phase {
        FixturePhase::Parse => scoopc::parser::parse_file(&source)
            .map(|_| ())
            .map_err(box_diagnostic),
        FixturePhase::Resolve => resolve_fixture(session, &source),
    };

    match (exp.expect, result) {
        (Expect::Pass, Ok(())) => Ok(()),
        (Expect::Pass, Err(e)) => Err(miette!("期望通过，但执行失败：{e}")),
        (Expect::Fail, Ok(())) => Err(miette!("期望失败，但执行成功")),
        (Expect::Fail, Err(e)) => {
            assert_diagnostic_matches(&source, &exp, &*e)?;
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixturePhase {
    Parse,
    Resolve,
}

fn resolve_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let ast = scoopc::parser::parse_file(source).map_err(box_diagnostic)?;

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;
    scoopc::resolve::check_file_bindings(source, &ast, &index).map_err(box_diagnostic)?;
    Ok(())
}

fn box_diagnostic<E>(e: E) -> Box<dyn miette::Diagnostic>
where
    E: miette::Diagnostic + 'static,
{
    Box::new(e)
}

fn assert_diagnostic_matches(
    source: &scoopc::source::SourceFile,
    exp: &FixtureExpectation<'_>,
    diag: &dyn miette::Diagnostic,
) -> Result<()> {
    if let Some(expected_code) = exp.error_code {
        let actual_code = diag.code().map(|c| c.to_string());
        if actual_code.as_deref() != Some(expected_code) {
            return Err(miette!(
                "错误码不匹配：期望 {expected_code:?}，实际为：{actual_code:?}"
            ));
        }
    }

    if let Some((line, col)) = exp.error_at {
        let (actual_line, actual_col) = primary_label_line_col(source, diag)?;
        if (actual_line, actual_col) != (line, col) {
            return Err(miette!(
                "错误位置不匹配：期望 {line}:{col}，实际为：{actual_line}:{actual_col}"
            ));
        }
    }

    if let Some(needle) = exp.error_contains {
        let msg = diag.to_string();
        if !msg.contains(needle) {
            return Err(miette!(
                "错误信息不匹配：期望包含 {needle:?}，实际为：{msg}"
            ));
        }
    }

    Ok(())
}

fn primary_label_line_col(
    source: &scoopc::source::SourceFile,
    diag: &dyn miette::Diagnostic,
) -> Result<(usize, usize)> {
    let mut first = None;
    let mut primary = None;

    if let Some(labels) = diag.labels() {
        for l in labels {
            first.get_or_insert(l.offset());
            if l.primary() {
                primary = Some(l.offset());
                break;
            }
        }
    }

    let offset = primary.or(first).ok_or_else(|| miette!("诊断未提供 labels/span，无法断言错误位置"))?;
    source.offset_to_line_col(offset)
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

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
