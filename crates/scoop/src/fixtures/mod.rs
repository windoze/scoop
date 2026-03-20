//! Fixtures 运行器（`scoop test`）。
//!
//! 设计目标：
//! - fixtures 是 Scoop 实现正确性的“底座”，必须可长期维护
//! - 每个 `.scoop` 文件可通过注释声明期望（pass/fail、错误包含等）
//!
//! 当前阶段支持：
//! - parse fixtures（调用 `scoopc::parser::parse_file`）
//! - resolve fixtures（最小名字绑定：import + TypeRef 解析）
//! - typecheck fixtures（T0403：TypeRef lowering + 泛型 arity 检查）
//! - run-pass fixtures：当前仅提供 stdout golden 比对逻辑与执行接口骨架（真实执行待后续任务接入）
//!
//! 目录路由（phase）：
//! - `tests/fixtures/parse/**` → parse
//! - `tests/fixtures/resolve/**` → resolve
//! - `tests/fixtures/resolve_multi/<case>/**` → resolve（多文件编译单元：按目录为单位）
//! - `tests/fixtures/codegen/**` / `tests/fixtures/run-pass/**` → run-pass
//! - 其它一级目录（如 `infer/`）会被识别为 phase，但目前统一返回“未实现”的诊断。

mod expectations;
mod run_pass;

use std::path::Component;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use miette::{Context as _, IntoDiagnostic as _, Result, miette};
use thiserror::Error;

use expectations::{Expect, FixtureExpectation};

pub fn run_all(fixtures_root: &Path) -> Result<usize> {
    // T0307：`resolve_multi/<case>/` 采用“目录作为编译单元”的形式，因此需要把这些 `.scoop`
    // 从单文件扫描里排除，并由专门的 case 运行器以“多文件 + 单一 index”方式执行。
    let resolve_multi_root = fixtures_root.join("resolve_multi");
    let resolve_multi_cases = collect_resolve_multi_cases(&resolve_multi_root)?;

    let mut files = Vec::new();
    collect_scoop_files(
        fixtures_root,
        &mut files,
        resolve_multi_root
            .is_dir()
            .then_some(resolve_multi_root.as_path()),
    )?;
    files.sort();

    if files.is_empty() && resolve_multi_cases.is_empty() {
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

    for case_dir in resolve_multi_cases {
        ok += run_resolve_multi_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("resolve_multi case 失败：{}", case_dir.display()))?;
    }

    Ok(ok)
}

fn run_one(session: &scoopc::session::Session, fixtures_root: &Path, path: &Path) -> Result<()> {
    let source = scoopc::source::SourceFile::load(path)?;
    let exp = FixtureExpectation::from_source(source.text());
    // T0102/T0107：当前仅解析 `// ARGS:`/`RUN-STDOUT`/`EXPECT-EXIT`/`TIMEOUT` 等指令并结构化存储，
    // 后续 phase/runner 再真正消费这些参数。
    let _ = exp.args.len();
    let _ = exp.run_stdout;
    let _ = exp.expect_exit;
    let _ = exp.timeout_ms;

    let rel = path.strip_prefix(fixtures_root).unwrap_or(path);
    let phase = match phase_dir(rel) {
        None => FixturePhase::Parse,
        Some(name) if name == "parse" || name == "spec_doctest" => FixturePhase::Parse,
        Some(name) if name == "resolve" => FixturePhase::Resolve,
        Some(name) if name == "typecheck" => FixturePhase::Typecheck,
        Some(name) if name == "codegen" || name == "run-pass" => FixturePhase::RunPass,
        Some(other) => FixturePhase::Unimplemented(other.to_string_lossy().to_string()),
    };

    let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = match phase {
        FixturePhase::Parse => parse_fixture(&source, path, &exp),
        FixturePhase::Resolve => resolve_fixture(session, &source),
        FixturePhase::Typecheck => typecheck_fixture(session, &source),
        FixturePhase::RunPass => run_pass::run_fixture_unimplemented(rel, path, &exp),
        FixturePhase::Unimplemented(phase) => Err(box_diagnostic(UnimplementedPhase {
            phase,
            fixture: rel.display().to_string(),
        })),
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum FixturePhase {
    Parse,
    Resolve,
    Typecheck,
    RunPass,
    Unimplemented(String),
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_multi case 需要至少 2 个 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_multi_case_too_small))]
struct ResolveMultiCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("fixtures phase `{phase}` 未实现（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::unimplemented_phase))]
struct UnimplementedPhase {
    phase: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 AST golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::ast_golden_read_failed))]
struct AstGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("AST snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::ast_golden_mismatch))]
struct AstGoldenMismatch {
    path: String,
    fixture: String,
}

fn parse_fixture(
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let ast = scoopc::parser::parse_file(source).map_err(box_diagnostic)?;

    let Some(golden_rel) = exp.ast_golden else {
        return Ok(());
    };

    let golden_path = fixture_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(golden_rel);

    let expected = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(AstGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;

    let actual = format!("{ast:#?}\n");
    let expected = normalize_newlines(&expected);
    let actual = normalize_newlines(&actual);

    if expected != actual {
        return Err(box_diagnostic(AstGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn resolve_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let mut ast = scoopc::parser::parse_file(source).map_err(box_diagnostic)?;

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;
    scoopc::resolve::check_file_bindings(source, &mut ast, &index).map_err(box_diagnostic)?;
    Ok(())
}

fn typecheck_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let mut ast = scoopc::parser::parse_file(source).map_err(box_diagnostic)?;

    // 先运行不依赖 resolver/index 的 typecheck 预检查：
    // - T0404：声明头类型注解的最小约束
    // - T0409：struct 字段声明的最小约束（重复字段、`var`、默认值）
    scoopc::typecheck::check_file_headers(source, &ast).map_err(box_diagnostic)?;
    scoopc::typecheck::check_file_struct_decls(source, &ast).map_err(box_diagnostic)?;

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;

    // typecheck phase 的前置条件：签名中的类型引用应当已 resolve（至少保证存在性/可见性）。
    let headers =
        scoopc::resolve::check_file_headers(source, &ast, &index).map_err(box_diagnostic)?;

    // T0406：表达式类型检查需要 resolver 在 AST 上写回 ValueIdent.resolved。
    // 因此 typecheck phase 在通过 headers 解析后，还需要进一步解析函数体/initializer（bodies）。
    //
    // 说明：
    // - 这里复用 resolver 的 block scope + value ident 解析逻辑（T0304/T0305/T0308）；
    // - 若 bodies 中存在未定义值引用，将以 resolve 错误提前失败（避免后续 typecheck 重复报错）。
    scoopc::resolve::check_file_bodies(source, &mut ast, &index, &headers).map_err(box_diagnostic)?;

    // 构建 type env：sysroot + 当前文件（用于 arity 检查与 nominal kind 判定）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot()).map_err(box_diagnostic)?;
    env.extend_from_file(source, &ast).map_err(box_diagnostic)?;

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    scoopc::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    scoopc::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    Ok(())
}

/// 运行一个 `tests/fixtures/resolve_multi/<case>/` 的多文件编译单元。
///
/// 规则（当前阶段）：
/// - case 目录下必须有 2+ 个 `.scoop` 文件
/// - 先把 case 内所有文件 + sysroot 一起构建 `Index`
/// - 再对 case 内每个文件分别运行 `check_file_bindings`，并按各自文件头注释断言 pass/fail
fn run_resolve_multi_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut paths = Vec::new();
    collect_scoop_files(case_dir, &mut paths, None)?;
    paths.sort();

    if paths.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(ResolveMultiCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    let mut sources = Vec::with_capacity(paths.len());
    let mut asts = Vec::with_capacity(paths.len());
    for path in &paths {
        let source = scoopc::source::SourceFile::load(path)?;
        let ast = scoopc::parser::parse_file(&source).map_err(miette::Report::new)?;
        sources.push(source);
        asts.push(ast);
    }

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (s, a) in sources.iter().zip(asts.iter()) {
        pairs.push((s, a));
    }

    let index = scoopc::resolve::Index::build(&pairs).map_err(miette::Report::new)?;

    for (source, ast) in sources.iter().zip(asts.iter_mut()) {
        let exp = FixtureExpectation::from_source(source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> =
            scoopc::resolve::check_file_bindings(source, ast, &index).map_err(box_diagnostic);

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(source, &exp, &*e)?;
            }
        }
    }

    Ok(paths.len())
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

    let offset = primary
        .or(first)
        .ok_or_else(|| miette!("诊断未提供 labels/span，无法断言错误位置"))?;
    source.offset_to_line_col(offset)
}

fn collect_scoop_files_inner(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    skip_dir: Option<&Path>,
) -> Result<()> {
    if skip_dir.is_some_and(|skip| dir.starts_with(skip)) {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            collect_scoop_files_inner(&path, out, skip_dir)?;
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>, skip_dir: Option<&Path>) -> Result<()> {
    collect_scoop_files_inner(dir, out, skip_dir)
}

fn collect_resolve_multi_cases(resolve_multi_root: &Path) -> Result<Vec<PathBuf>> {
    if !resolve_multi_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(resolve_multi_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", resolve_multi_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    // 稳定排序（便于定位错误）。
    cases.sort();
    Ok(cases)
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// 返回 fixture 的一级目录名（即 phase 目录）。
///
/// 例如：
/// - `parse/hello.scoop` → Some("parse")
/// - `resolve/foo/bar.scoop` → Some("resolve")
/// - `hello.scoop` → None（直接放在根目录下，按 parse 处理以保持兼容）
fn phase_dir(rel: &Path) -> Option<&std::ffi::OsStr> {
    let mut comps = rel.components();
    let first = comps.next();
    let second = comps.next();
    match (first, second) {
        (Some(Component::Normal(name)), Some(_)) => Some(name),
        _ => None,
    }
}
