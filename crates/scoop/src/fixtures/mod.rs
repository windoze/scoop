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
//! - infer fixtures（T05：类型推断阶段；当前先复用 typecheck pipeline，逐步打开更多推断能力）
//! - run-pass fixtures：当前仅提供 stdout/stderr golden 比对逻辑与执行接口骨架（真实执行待后续任务接入）
//!
//! 目录路由（phase）：
//! - `tests/fixtures/parse/**` → parse
//! - `tests/fixtures/resolve/**` → resolve
//! - `tests/fixtures/resolve_multi/<case>/**` → resolve（多文件编译单元：按目录为单位）
//! - `tests/fixtures/resolve_cone/<case>/<cone>/**` → resolve（多 cone：每个 cone 子目录作为独立可见性边界）
//! - `tests/fixtures/typecheck_multi/<case>/**` → typecheck（多文件编译单元：按目录为单位）
//! - `tests/fixtures/typecheck_cone/<case>/<cone>/**` → typecheck（多 cone：每个 cone 子目录作为独立可见性边界）
//! - `tests/fixtures/codegen/**` / `tests/fixtures/run-pass/**` → run-pass
//! - `tests/fixtures/infer/**` → infer
//! - `tests/fixtures/hir/**` → hir（HIR lowering + `.hir` golden 比对）
//! - `tests/fixtures/mir/**` → mir（MIR lowering + `.mir` golden 比对）
//! - 其它一级目录会被识别为 phase，但目前统一返回“未实现”的诊断。

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
    // T0321a：`resolve_cone/<case>/<cone>/` 用于模拟“多个 cone（包/依赖边界）”。
    let resolve_cone_root = fixtures_root.join("resolve_cone");
    let resolve_cone_cases = collect_resolve_cone_cases(&resolve_cone_root)?;
    let typecheck_multi_root = fixtures_root.join("typecheck_multi");
    let typecheck_multi_cases = collect_typecheck_multi_cases(&typecheck_multi_root)?;
    let typecheck_cone_root = fixtures_root.join("typecheck_cone");
    let typecheck_cone_cases = collect_typecheck_cone_cases(&typecheck_cone_root)?;

    let mut files = Vec::new();
    let mut skip_dirs: Vec<&Path> = Vec::new();
    if resolve_multi_root.is_dir() {
        skip_dirs.push(resolve_multi_root.as_path());
    }
    if resolve_cone_root.is_dir() {
        skip_dirs.push(resolve_cone_root.as_path());
    }
    if typecheck_multi_root.is_dir() {
        skip_dirs.push(typecheck_multi_root.as_path());
    }
    if typecheck_cone_root.is_dir() {
        skip_dirs.push(typecheck_cone_root.as_path());
    }
    collect_scoop_files(fixtures_root, &mut files, &skip_dirs)?;
    files.sort();

    if files.is_empty()
        && resolve_multi_cases.is_empty()
        && typecheck_multi_cases.is_empty()
        && typecheck_cone_cases.is_empty()
    {
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

    for case_dir in resolve_cone_cases {
        ok += run_resolve_cone_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("resolve_cone case 失败：{}", case_dir.display()))?;
    }

    for case_dir in typecheck_multi_cases {
        ok += run_typecheck_multi_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("typecheck_multi case 失败：{}", case_dir.display()))?;
    }

    for case_dir in typecheck_cone_cases {
        ok += run_typecheck_cone_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("typecheck_cone case 失败：{}", case_dir.display()))?;
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
    let _ = exp.run_stderr;
    let _ = exp.expect_exit;
    let _ = exp.timeout_ms;

    let rel = path.strip_prefix(fixtures_root).unwrap_or(path);
    let phase = match phase_dir(rel) {
        None => FixturePhase::Parse,
        Some(name) if name == "parse" || name == "spec_doctest" => FixturePhase::Parse,
        Some(name) if name == "resolve" => FixturePhase::Resolve,
        Some(name) if name == "typecheck" => FixturePhase::Typecheck,
        Some(name) if name == "infer" => FixturePhase::Infer,
        Some(name) if name == "codegen" || name == "run-pass" => FixturePhase::RunPass,
        Some(name) if name == "hir" => FixturePhase::Hir,
        Some(name) if name == "mir" => FixturePhase::Mir,
        Some(other) => FixturePhase::Unimplemented(other.to_string_lossy().to_string()),
    };

    let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = match phase {
        FixturePhase::Parse => parse_fixture(&source, path, &exp),
        FixturePhase::Resolve => resolve_fixture(session, &source),
        FixturePhase::Typecheck => typecheck_fixture(session, &source),
        FixturePhase::Infer => infer_fixture(session, &source),
        FixturePhase::RunPass => run_pass::run_fixture_unimplemented(rel, path, &exp),
        FixturePhase::Hir => hir_fixture(session, &source, path),
        FixturePhase::Mir => mir_fixture(session, &source, path),
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
    Infer,
    RunPass,
    Hir,
    Mir,
    Unimplemented(String),
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_multi case 需要至少 2 个 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_multi_case_too_small))]
struct ResolveMultiCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_cone case 需要至少 2 个 cone 子目录（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_cone_case_too_small))]
struct ResolveConeCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_cone case 的 cone 子目录 `{cone}` 下未发现任何 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_cone_cone_empty))]
struct ResolveConeConeEmpty {
    fixture: String,
    cone: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_multi case 需要至少 2 个 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_multi_case_too_small))]
struct TypecheckMultiCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_cone case 需要至少 2 个 cone 子目录（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_cone_case_too_small))]
struct TypecheckConeCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_cone case 的 cone 子目录 `{cone}` 下未发现任何 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_cone_cone_empty))]
struct TypecheckConeConeEmpty {
    fixture: String,
    cone: String,
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

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 HIR golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::hir_golden_read_failed))]
struct HirGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("HIR snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::hir_golden_mismatch))]
struct HirGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 MIR golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::mir_golden_read_failed))]
struct MirGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("MIR snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::mir_golden_mismatch))]
struct MirGoldenMismatch {
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

fn hir_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let lowered = scoopc::hir::lower_for_dump(session, source).map_err(box_diagnostic)?;
    let actual = normalize_newlines(&format!("{:#?}\n", lowered.file));

    let golden_path = fixture_path.with_extension("hir");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(HirGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(HirGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn mir_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let lowered = scoopc::mir::lower_for_dump(session, source).map_err(box_diagnostic)?;
    let actual = normalize_newlines(&format!("{:#?}\n", lowered.file));

    let golden_path = fixture_path.with_extension("mir");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(MirGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(MirGoldenMismatch {
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

    // 构建 type env：sysroot + 当前文件（用于跨文件 type position 查询）。
    let mut env =
        scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).map_err(box_diagnostic)?;
    env.extend_from_file(source, &ast, &index)
        .map_err(box_diagnostic)?;

    // T0431/T0432：属性（class/value type）的最小语义检查。
    scoopc::typecheck::check_file_properties(source, &ast, &index, &env).map_err(box_diagnostic)?;
    // T0439：class 继承与 override 的最小语义检查。
    scoopc::typecheck::check_file_inheritance(source, &ast, &index).map_err(box_diagnostic)?;

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    // T0440：interface 实现列表 + 抽象成员实现检查（默认方法不要求实现）。
    scoopc::typecheck::check_file_interfaces(source, &ast, &index, &env).map_err(box_diagnostic)?;
    // T0609：override/interface impl 的 effect row 不能增加（R_over ⊆ R_base）。
    scoopc::typecheck::check_file_override_effects(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

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

    scoopc::typecheck::check_file_where_clauses(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    scoopc::typecheck::check_file_overload_conflicts(
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

    // T0449：计算 enum/Option 的布局元数据（niche/boxing/lint）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(box_diagnostic)?;

    Ok(())
}

fn infer_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // 当前阶段（T0502）先让 `infer` fixtures 走与 `typecheck` 相同的 pipeline：
    // - 便于把“依赖推断的新用例”与既有 typecheck fixtures 逻辑隔离
    // - 后续在这里逐步接入 constraint generation + solving
    //
    // 说明：这不是“重复执行”，而是为 T05 预留独立入口。
    typecheck_fixture(session, source)
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
    collect_scoop_files(case_dir, &mut paths, &[])?;
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

/// 运行一个 `tests/fixtures/resolve_cone/<case>/<cone>/` 的“多 cone”用例。
///
/// 规则（当前阶段，T0321a）：
/// - case 目录下必须有 2+ 个 cone 子目录（每个子目录代表一个 cone/依赖边界）
/// - 每个 cone 子目录下至少有 1 个 `.scoop` 文件
/// - 将所有 cone 的文件 + sysroot 一起构建 `Index`（但每个文件携带不同的 cone id）
/// - 对 cone 内每个文件分别运行 `check_file_bindings`，并按各自文件头注释断言 pass/fail
fn run_resolve_cone_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut cone_dirs = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", case_dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_dir() {
            cone_dirs.push(entry.path());
        }
    }

    cone_dirs.sort();
    if cone_dirs.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(ResolveConeCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    struct ConeFile {
        cone: scoopc::resolve::ConeId,
        source: scoopc::source::SourceFile,
        ast: scoopc::ast::File,
    }

    let mut files: Vec<ConeFile> = Vec::new();
    let mut ok = 0usize;

    for (idx, cone_dir) in cone_dirs.iter().enumerate() {
        let mut paths = Vec::new();
        collect_scoop_files(cone_dir, &mut paths, &[])?;
        paths.sort();

        if paths.is_empty() {
            let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
            let cone = cone_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            return Err(ResolveConeConeEmpty {
                fixture: rel.display().to_string(),
                cone,
            }
            .into());
        }

        // cone id 0 保留给 sysroot；fixture 的 cone 从 1 开始稳定分配（按目录名排序）。
        let cone_id = scoopc::resolve::ConeId::new((idx as u32) + 1);

        for path in paths {
            let source = scoopc::source::SourceFile::load(&path)?;
            let ast = scoopc::parser::parse_file(&source).map_err(miette::Report::new)?;
            files.push(ConeFile {
                cone: cone_id,
                source,
                ast,
            });
            ok += 1;
        }
    }

    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for f in &files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: f.cone,
            source: &f.source,
            file: &f.ast,
        });
    }

    let index = scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::new)?;

    for f in files.iter_mut() {
        let exp = FixtureExpectation::from_source(f.source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> =
            scoopc::resolve::check_file_bindings(&f.source, &mut f.ast, &index).map_err(box_diagnostic);

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(&f.source, &exp, &*e)?;
            }
        }
    }

    Ok(ok)
}

/// 运行一个 `tests/fixtures/typecheck_cone/<case>/<cone>/` 的“多 cone”用例。
///
/// 规则（当前阶段，T0629a）：
/// - case 目录下必须有 2+ 个 cone 子目录（每个子目录代表一个 cone/依赖边界）
/// - 每个 cone 子目录下至少有 1 个 `.scoop` 文件
/// - 将所有 cone 的文件 + sysroot 一起构建 `Index`（每个文件携带不同的 cone id）
/// - 构建 type env：sysroot + 全部 cone 的文件（用于跨 cone 的 TypeRef lowering）
/// - 对 cone 内每个文件分别运行 typecheck pipeline，并按各自文件头注释断言 pass/fail
fn run_typecheck_cone_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut cone_dirs = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", case_dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_dir() {
            cone_dirs.push(entry.path());
        }
    }

    cone_dirs.sort();
    if cone_dirs.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckConeCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    struct ConeFile {
        cone: scoopc::resolve::ConeId,
        source: scoopc::source::SourceFile,
        ast: scoopc::ast::File,
    }

    let mut files: Vec<ConeFile> = Vec::new();
    let mut ok = 0usize;

    for (idx, cone_dir) in cone_dirs.iter().enumerate() {
        let mut paths = Vec::new();
        collect_scoop_files(cone_dir, &mut paths, &[])?;
        paths.sort();

        if paths.is_empty() {
            let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
            let cone = cone_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            return Err(TypecheckConeConeEmpty {
                fixture: rel.display().to_string(),
                cone,
            }
            .into());
        }

        // cone id 0 保留给 sysroot；fixture 的 cone 从 1 开始稳定分配（按目录名排序）。
        let cone_id = scoopc::resolve::ConeId::new((idx as u32) + 1);

        for path in paths {
            let source = scoopc::source::SourceFile::load(&path)?;
            let ast = scoopc::parser::parse_file(&source).map_err(miette::Report::new)?;
            files.push(ConeFile {
                cone: cone_id,
                source,
                ast,
            });
            ok += 1;
        }
    }

    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for f in &files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: f.cone,
            source: &f.source,
            file: &f.ast,
        });
    }

    let index = scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::new)?;

    // type env：sysroot + 全部 cone 的文件（用于跨 cone TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::new)?;
    for f in &files {
        env.extend_from_file(&f.source, &f.ast, &index)
            .map_err(miette::Report::new)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    for f in files.iter_mut() {
        let exp = FixtureExpectation::from_source(f.source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
            // 先运行不依赖 resolver/index 的 typecheck 预检查。
            scoopc::typecheck::check_file_headers(&f.source, &mut f.ast).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_struct_decls(&f.source, &mut f.ast)
                .map_err(box_diagnostic)?;

            // resolver phase：headers + bodies。
            let headers =
                scoopc::resolve::check_file_headers(&f.source, &mut f.ast, &index).map_err(box_diagnostic)?;
            scoopc::resolve::check_file_bodies(&f.source, &mut f.ast, &index, &headers)
                .map_err(box_diagnostic)?;

            // typecheck phase。
            scoopc::typecheck::check_file_properties(&f.source, &mut f.ast, &index, &env)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_inheritance(&f.source, &mut f.ast, &index)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_interfaces(&f.source, &mut f.ast, &index, &env)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_override_effects(
                &f.source,
                &mut f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_type_refs(
                &f.source,
                &mut f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_where_clauses(
                &f.source,
                &mut f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_overload_conflicts(
                &f.source,
                &mut f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_exprs(
                &f.source,
                &mut f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            Ok(())
        })();

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(&f.source, &exp, &*e)?;
            }
        }
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 typecheck_multi 对齐）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::new)?;

    Ok(ok)
}

/// 运行一个 `tests/fixtures/typecheck_multi/<case>/` 的多文件编译单元。
///
/// 规则（当前阶段）：
/// - case 目录下必须有 2+ 个 `.scoop` 文件
/// - 先把 case 内所有文件 + sysroot 一起构建 `Index`
/// - 构建 type env：sysroot + case 全部文件（用于跨文件的 TypeRef lowering / arity 检查）
/// - 再对 case 内每个文件分别运行 typecheck pipeline，并按各自文件头注释断言 pass/fail
fn run_typecheck_multi_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut paths = Vec::new();
    collect_scoop_files(case_dir, &mut paths, &[])?;
    paths.sort();

    if paths.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckMultiCaseTooSmall {
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

    // 先构建单一 Index（sysroot + case）。
    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (s, a) in sources.iter().zip(asts.iter()) {
        pairs.push((s, a));
    }
    let index = scoopc::resolve::Index::build(&pairs).map_err(miette::Report::new)?;

    // type env：sysroot + case 全部文件（用于跨文件 TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::new)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::new)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    for (source, ast) in sources.iter().zip(asts.iter_mut()) {
        let exp = FixtureExpectation::from_source(source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
            // 先运行不依赖 resolver/index 的 typecheck 预检查。
            scoopc::typecheck::check_file_headers(source, ast).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_struct_decls(source, ast).map_err(box_diagnostic)?;

            // resolver phase：headers + bodies。
            let headers =
                scoopc::resolve::check_file_headers(source, ast, &index).map_err(box_diagnostic)?;
            scoopc::resolve::check_file_bodies(source, ast, &index, &headers)
                .map_err(box_diagnostic)?;

            // typecheck phase。
            scoopc::typecheck::check_file_properties(source, ast, &index, &env).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_inheritance(source, ast, &index).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_interfaces(source, ast, &index, &env).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_override_effects(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_type_refs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_where_clauses(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_overload_conflicts(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            Ok(())
        })();

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(source, &exp, &*e)?;
            }
        }
    }

    // T0449：对整个编译单元中出现过的类型做一次 layout/metadata 计算。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::new)?;

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
    skip_dirs: &[&Path],
) -> Result<()> {
    if skip_dirs.iter().any(|skip| dir.starts_with(skip)) {
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
            collect_scoop_files_inner(&path, out, skip_dirs)?;
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>, skip_dirs: &[&Path]) -> Result<()> {
    collect_scoop_files_inner(dir, out, skip_dirs)
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

fn collect_resolve_cone_cases(resolve_cone_root: &Path) -> Result<Vec<PathBuf>> {
    if !resolve_cone_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(resolve_cone_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", resolve_cone_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_typecheck_multi_cases(typecheck_multi_root: &Path) -> Result<Vec<PathBuf>> {
    if !typecheck_multi_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(typecheck_multi_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", typecheck_multi_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_typecheck_cone_cases(typecheck_cone_root: &Path) -> Result<Vec<PathBuf>> {
    if !typecheck_cone_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(typecheck_cone_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", typecheck_cone_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

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
