//! `scoop build` 子命令。
//!
//! T0805：实现“前端检查 + 输出路径准备”。
//!
//! T0806：在启用 `scoop` 的 LLVM 后端时（默认开启；可用 `--no-default-features` 关闭），额外执行：
//! - 生成最小 object（当前阶段仍是固定 `main → ret 0`）；
//! - 调用 clang 链接 object + 早期 C runtime，产出可执行文件。

mod deps;

use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use thiserror::Error;

#[derive(Debug)]
struct BuildInput {
    /// 当前编译单元的全部源文件。
    ///
    /// 约定（T1315a）：
    /// - 始终包含 stdlib 注入的 `stdlib/*.scoop`（纯 Scoop prelude）；
    /// - 单文件模式下额外包含 1 个 user source；
    /// - cone 包模式下额外包含 `src/**/*.scoop`。
    sources: Vec<scoopc::source::SourceFile>,
    /// 可执行入口（选定的 `fun main` 所在源文件）在 `sources` 中的下标。
    main_index: usize,
    /// cone 包模式下的“锚点 main 文件”（`src/main.scoop`）在 `sources` 中的下标。
    ///
    /// 用途：
    /// - 当未显式配置 `entry-package` 时，用它的 package 作为默认 entry package；
    /// - 未来其它 driver/fixture 逻辑也可以用它作为“case 的稳定入口文件”。
    cone_anchor_main_index: Option<usize>,
    /// 若输入为 cone 包目录，则包含其 root 与 manifest（用于 T1107 依赖图解析）。
    cone_root: Option<PathBuf>,
    cone_manifest: Option<scoopc::cone::ConeManifest>,
    /// （cone 包模式）入口 package 覆盖（来自 CLI）。
    entry_package_override: Option<String>,
    /// 已选择的入口函数 FQN（仅 cone 包模式下会填充）。
    entry_main_fqn: Option<String>,
}

impl BuildInput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        &self.sources[self.main_index]
    }

    fn cone_anchor_main_source(&self) -> Option<&scoopc::source::SourceFile> {
        self.cone_anchor_main_index.map(|idx| &self.sources[idx])
    }
}

#[derive(Debug)]
struct FrontendOutput {
    input: BuildInput,
    #[cfg(feature = "llvm")]
    asts: Vec<scoopc::ast::File>,
    #[cfg(feature = "llvm")]
    index: scoopc::resolve::Index,
}

impl FrontendOutput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        self.input.main_source()
    }

    #[cfg(feature = "llvm")]
    fn main_ast(&self) -> &scoopc::ast::File {
        &self.asts[self.input.main_index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEmit {
    /// 产出可执行文件（默认）。
    Executable,
    /// 产出 LLVM IR（`.ll`）。
    LlvmIr,
    /// 产出 object 文件（`.o` / `.obj`）。
    Obj,
    /// 产出汇编（`.s` / `.asm`）。
    Asm,
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub emit: BuildEmit,
    /// （cone 包模式）入口 package（覆盖 `Cone.toml` 的 `native-build.entry-package`）。
    pub entry_package: Option<String>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            emit: BuildEmit::Executable,
            entry_package: None,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("`--entry-package` 仅支持 cone 包目录输入：{input}")]
#[diagnostic(code(scoop::driver::entry_package_only_for_cone))]
pub(crate) struct EntryPackageOnlyForCone {
    input: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("入口包 `{entry_package}` 中找不到入口函数 `fun main`")]
#[diagnostic(code(scoop::driver::entry_package_missing_main))]
pub(crate) struct EntryPackageMissingMain {
    entry_package: String,
    #[label("该 package 没有 `main`")]
    span: miette::SourceSpan,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "入口包 `{entry_package}` 的 `fun main` 不属于 consumer cone（它声明在依赖/其它 cone：{decl_file}）"
)]
#[diagnostic(code(scoop::driver::entry_package_main_not_in_consumer_cone))]
pub(crate) struct EntryPackageMainNotInConsumerCone {
    entry_package: String,
    decl_file: String,
}

/// 执行 `scoop build <input> [-o <output>]`。
///
/// 当前阶段验收点：
/// - 输入可通过 parse/resolve/typecheck 时返回 `Ok(())`；
/// - 当启用 LLVM 后端时（默认已启用；若你用了 `--no-default-features` 则需要显式开启）：
///   - 默认产出可执行文件；
///   - 若指定 `--emit-llvm/--emit-obj/--emit-asm`，则改为产出对应单文件产物。
pub fn run(input: PathBuf, output: Option<PathBuf>, options: BuildOptions) -> Result<()> {
    let BuildOptions {
        emit,
        entry_package,
    } = options;

    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = output.unwrap_or_else(|| default_output_path_for_emit(emit));
    ensure_output_parent_dir(&output)?;

    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    let session = scoopc::session::Session::new()?;

    let input = load_build_input(&input, entry_package)?;
    let deps = match (&input.cone_root, &input.cone_manifest) {
        (Some(root), Some(manifest)) => deps::load_dependency_graph(manifest, root)?,
        _ => Vec::new(),
    };
    let front = run_frontend(&session, input, &deps)?;
    // 非 llvm 构建下，codegen 分支会被编译掉；这里显式访问一次 main 以避免 dead_code 警告，
    // 同时也作为“加载逻辑能稳定定位入口”的最小一致性校验。
    let _ = front.main_source();

    match emit {
        BuildEmit::Executable => {
            // 只有在启用 LLVM 后端时才会真正生成可执行文件；默认构建仍保持“前端检查”可用。
            #[cfg(feature = "llvm")]
            run_codegen_and_link(&session, &front, &output)?;
        }
        BuildEmit::LlvmIr => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_ir_to_file_from_lowered_hir_with_entry(
                    front.main_source(),
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-llvm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-llvm <file> -o <out.ll>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
        BuildEmit::Obj => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir_with_entry(
                    front.main_source(),
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-obj` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-obj <file> -o <out.o>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
        BuildEmit::Asm => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_asm_to_file_from_lowered_hir_with_entry(
                    front.main_source(),
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-asm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-asm <file> -o <out.s>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
    }

    Ok(())
}

fn load_build_input(input: &Path, entry_package_override: Option<String>) -> Result<BuildInput> {
    let stdlib_sources = load_stdlib_sources()?;

    // 单文件模式：保持 `scoop build <file.scoop>` 的原有行为。
    if input.is_file() {
        if entry_package_override.is_some() {
            return Err(EntryPackageOnlyForCone {
                input: input.display().to_string(),
            }
            .into());
        }
        let mut sources = stdlib_sources;
        let main_index = sources.len();
        sources.push(scoopc::source::SourceFile::load(input)?);
        return Ok(BuildInput {
            sources,
            main_index,
            cone_anchor_main_index: None,
            cone_root: None,
            cone_manifest: None,
            entry_package_override: None,
            entry_main_fqn: None,
        });
    }

    // cone 包模式：`scoop build <cone-root>`（按 T1102 规则定位 `src/main.scoop`）。
    if input.is_dir() {
        let pkg = scoopc::cone::load_cone_source_package(input)?;
        let mut sources = stdlib_sources;
        let stdlib_len = sources.len();
        sources.reserve(pkg.sources.len());
        let mut main_index = None;
        for (idx, path) in pkg.sources.iter().enumerate() {
            let source = scoopc::source::SourceFile::load(path)?;
            if source.path() == pkg.main.as_path() {
                main_index = Some(stdlib_len + idx);
            }
            sources.push(source);
        }

        let main_index = main_index.ok_or_else(|| {
            miette::miette!(
                "cone package 的 main 未出现在 sources 列表中：{}",
                pkg.main.display()
            )
        })?;

        return Ok(BuildInput {
            sources,
            main_index,
            cone_anchor_main_index: Some(main_index),
            cone_root: Some(pkg.root),
            cone_manifest: Some(pkg.manifest),
            entry_package_override,
            entry_main_fqn: None,
        });
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

fn default_stdlib_path() -> PathBuf {
    // 开发期路径：相对于 `crates/scoop` 的 `../../stdlib`。
    // 后续可随“工具链安装/分发”演进为：
    // - `SCOOP_STDLIB` 环境变量
    // - 或可执行文件旁的资源目录
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn load_stdlib_sources() -> Result<Vec<scoopc::source::SourceFile>> {
    let root = default_stdlib_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 stdlib 目录（T1315a）")?;

    let mut paths = Vec::new();
    collect_scoop_files(&root, &mut paths)?;
    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        out.push(scoopc::source::SourceFile::load(&path)?);
    }
    Ok(out)
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

fn run_frontend(
    session: &scoopc::session::Session,
    mut input: BuildInput,
    deps: &[scoopc::cone::ConeArchiveApi],
) -> Result<FrontendOutput> {
    if input.sources.is_empty() {
        return Err(miette::miette!("内部错误：build 输入 sources 为空"));
    }

    // 先 parse 所有文件（cone 包模式下：`src/**/*.scoop`）。
    let mut asts = Vec::with_capacity(input.sources.len());
    for source in &input.sources {
        let ast = scoopc::parser::parse_file(source).map_err(miette::Report::from)?;
        asts.push(ast);
    }

    // 先运行不依赖 resolver/index 的 typecheck 预检查（与 fixtures/typecheck pipeline 对齐）。
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        scoopc::typecheck::check_file_headers(source, ast).map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_struct_decls(source, ast).map_err(miette::Report::from)?;
    }

    // 构建 Index：sysroot 作为 cone 0；当前被 build 的 cone 作为 cone 1。
    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(1),
            source,
            file: ast,
        });
    }

    let mut index =
        scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // T0629b：program boundary 的“库导出入口 / host entry points”由 Cone.toml 指定，
    // 在 typecheck 阶段按 entry point 规则强制 `Pure!`。
    if let Some(manifest) = input.cone_manifest.as_ref() {
        index.set_export_entry_points(manifest.export_entry_points.clone());
    }

    // T1107：注入 `.cone` 依赖的 public API（用于 import/类型检查）。
    //
    // cone id 分配约定：
    // - 0：sysroot
    // - 1：当前被 build 的 cone（consumer）
    // - 2+：按依赖拓扑序分配（deps 由 build/deps.rs 负责解析为 DAG 顺序）
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    let mut next_dep_cone: u32 = 2;
    for dep in deps {
        let dep_cone = scoopc::resolve::ConeId::new(next_dep_cone);
        next_dep_cone += 1;
        scoopc::cone::inject_cone_dependency_public_api(&mut index, &mut env, dep_cone, dep)
            .map_err(miette::Report::from)?;
    }

    // T1113：选择“入口包”的 `fun main`，并将其作为 runtime entry point。
    //
    // 说明：
    // - 该选择仅在 cone 包模式下生效；单文件模式保持现有行为。
    // - 该选择会影响：
    //   - typecheck：仅对选定 `main` 按 entry point 规则强制 `Pure!`；
    //   - HIR lowering / LLVM codegen：选定 `main` 所在文件作为 entry source（允许 source-backed literals）。
    if input.cone_manifest.is_some() {
        select_cone_entry_main(&mut input, &asts, &mut index)?;
    }

    // resolver phase：headers + bodies（逐文件运行，但共享同一个 index）。
    let mut headers = Vec::with_capacity(input.sources.len());
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        let h = scoopc::resolve::check_file_headers(source, ast, &index)
            .map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in input
        .sources
        .iter()
        .zip(asts.iter_mut())
        .zip(headers.iter())
    {
        scoopc::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    // type env：sysroot + 依赖 cones（已注入）+ 当前 cone 全部文件（用于跨文件 TypeRef lowering）。
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    // typecheck phase：逐文件执行（共享 env/index/types）。
    for ((source, ast), h) in input.sources.iter().zip(asts.iter()).zip(headers.iter()) {
        scoopc::typecheck::check_file_annotations(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_properties(source, ast, &index, &env)
            .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_inheritance(source, ast, &index)
            .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_interfaces(source, ast, &index, &env)
            .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_override_effects(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_type_refs(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_where_clauses(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_overload_conflicts(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_exprs(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 fixtures/typecheck_multi 对齐）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;

    Ok(FrontendOutput {
        input,
        #[cfg(feature = "llvm")]
        asts,
        #[cfg(feature = "llvm")]
        index,
    })
}

fn package_prefix(
    source: &scoopc::source::SourceFile,
    pkg: Option<&scoopc::ast::PackageDecl>,
) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

fn cone_entry_main_fqn(entry_package: &str) -> String {
    if entry_package.is_empty() {
        "main".to_string()
    } else {
        format!("{entry_package}.main")
    }
}

fn find_consumer_package_decl_span(
    input: &BuildInput,
    asts: &[scoopc::ast::File],
    entry_package: &str,
) -> miette::SourceSpan {
    let Some(root) = input.cone_root.as_ref() else {
        return scoopc::span::Span::new(0, 0).into();
    };

    for (source, file) in input.sources.iter().zip(asts.iter()) {
        if !source.path().starts_with(root) {
            continue;
        }
        let Some(pkg) = file.package.as_ref() else {
            continue;
        };
        if package_prefix(source, Some(pkg)) == entry_package {
            return pkg.span.into();
        }
    }

    // fallback：锚点 main 文件的 package（如果存在）。
    if let Some(anchor) = input.cone_anchor_main_index {
        if let Some(pkg) = asts.get(anchor).and_then(|file| file.package.as_ref()) {
            return pkg.span.into();
        }
    }

    scoopc::span::Span::new(0, 0).into()
}

fn select_cone_entry_main(
    input: &mut BuildInput,
    asts: &[scoopc::ast::File],
    index: &mut scoopc::resolve::Index,
) -> Result<()> {
    let Some(manifest) = input.cone_manifest.as_ref() else {
        return Ok(());
    };

    let entry_package = if let Some(v) = input.entry_package_override.as_deref() {
        v.trim().to_string()
    } else if let Some(v) = manifest.native_build.entry_package.as_deref() {
        v.trim().to_string()
    } else {
        let anchor = input.cone_anchor_main_index.unwrap_or(input.main_index);
        let anchor_source = &input.sources[anchor];
        let anchor_file = &asts[anchor];
        package_prefix(anchor_source, anchor_file.package.as_ref())
    };

    let entry_main_fqn = cone_entry_main_fqn(&entry_package);
    index.set_runtime_entry_point(entry_main_fqn.clone());
    input.entry_main_fqn = Some(entry_main_fqn.clone());

    // 约定（与本模块 build pipeline 对齐）：
    // - cone 0：sysroot
    // - cone 1：consumer（当前被 build 的 cone）
    // - cone 2+：依赖 cones
    let consumer_cone = scoopc::resolve::ConeId::new(1);

    let overload_in_consumer = index.by_fqn.get(&entry_main_fqn).and_then(|syms| {
        syms.fun.iter().find(|o| {
            o.symbol.decl_cone == consumer_cone
                && o.sig.receiver.is_none()
                && o.sig.params.is_empty()
        })
    });

    if let Some(overload) = overload_in_consumer {
        let decl_file = overload.symbol.decl_file.as_path();
        let Some((idx, _)) = input
            .sources
            .iter()
            .enumerate()
            .find(|(_idx, s)| s.path() == decl_file)
        else {
            return Err(miette::miette!(
                "内部错误：入口 main 源文件未在 sources 列表中：{}",
                decl_file.display()
            ));
        };

        input.main_index = idx;
        return Ok(());
    }

    if let Some(syms) = index.by_fqn.get(&entry_main_fqn) {
        if let Some(overload) = syms.fun.first() {
            if overload.symbol.decl_cone != consumer_cone {
                return Err(EntryPackageMainNotInConsumerCone {
                    entry_package,
                    decl_file: overload.symbol.decl_file.display().to_string(),
                }
                .into());
            }
        }
    }

    let span = find_consumer_package_decl_span(input, asts, &entry_package);
    Err(EntryPackageMissingMain {
        entry_package,
        span,
    }
    .into())
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

fn default_output_path_for_emit(emit: BuildEmit) -> PathBuf {
    match emit {
        BuildEmit::Executable => {
            let ext = std::env::consts::EXE_EXTENSION;
            if ext.is_empty() {
                PathBuf::from("a.out")
            } else {
                PathBuf::from(format!("a.{ext}"))
            }
        }
        BuildEmit::LlvmIr => PathBuf::from("a.ll"),
        BuildEmit::Obj => {
            if cfg!(windows) {
                PathBuf::from("a.obj")
            } else {
                PathBuf::from("a.o")
            }
        }
        BuildEmit::Asm => {
            if cfg!(windows) {
                PathBuf::from("a.asm")
            } else {
                PathBuf::from("a.s")
            }
        }
    }
}

#[cfg(feature = "llvm")]
fn run_codegen_and_link(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    output: &Path,
) -> Result<()> {
    let dir = super::temp::make_temp_dir("scoop_build")?;
    let obj = dir.join("main.o");

    let lowered = lower_main_hir_for_build(session, front)?;
    scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir_with_entry(
        front.main_source(),
        &lowered,
        &obj,
        front.input.entry_main_fqn.as_deref(),
    )?;

    // T1114：把 Cone.toml 的 `native-build.linker/link-flags` 透传到最终链接命令。
    let options = crate::toolchain::LinkOptions {
        linker: front
            .input
            .cone_manifest
            .as_ref()
            .and_then(|m| m.native_build.linker.as_deref()),
        link_flags: front
            .input
            .cone_manifest
            .as_ref()
            .map(|m| m.native_build.link_flags.as_slice())
            .unwrap_or(&[]),
    };
    crate::toolchain::link_obj_with_runtime(&obj, output, &lowered.extern_libs, options)?;

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[cfg(feature = "llvm")]
fn lower_main_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
) -> Result<scoopc::hir::LoweredHir> {
    // compilation unit：sysroot + 当前 cone 全部源文件（稳定顺序）。
    let mut unit: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        unit.push((&f.source, &f.ast));
    }
    for (source, ast) in front.input.sources.iter().zip(front.asts.iter()) {
        unit.push((source, ast));
    }

    // T1315a：stdlib 注入后需要 multi-file lowering（否则 stdlib 顶层函数不会出现在 fun_index 中）。
    // 说明：当前 LLVM 后端仍以“单一 SourceFile”读取字面量文本；multi-file lowering 会拒绝
    // 非入口文件中的源文本字面量（见 `scoopc::hir::lower_for_compilation_unit_multi_files`）。
    let files_to_lower = front
        .input
        .sources
        .iter()
        .zip(front.asts.iter())
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    scoopc::hir::lower_for_compilation_unit_multi_files(
        front.main_source(),
        &front.index,
        &unit,
        &files_to_lower,
    )
    .map_err(miette::Report::from)
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

        super::run(input, Some(out), super::BuildOptions::default()).unwrap();
        assert!(dir.path().join("nested").is_dir());
    }

    #[test]
    fn build_accepts_cone_package_dir_and_finds_main() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-pkg"
version = "0.0.0"
"#,
        )
        .unwrap();

        std::fs::write(src.join("main.scoop"), "fun main() {}\n").unwrap();
        std::fs::write(src.join("util.scoop"), "fun helper() {}\n").unwrap();

        let out = dir.path().join("out").join("a");
        super::run(pkg, Some(out), super::BuildOptions::default()).unwrap();
    }

    #[test]
    fn build_cone_package_can_load_cone_deps_for_frontend() {
        let dir = tempdir().unwrap();

        // 1) 准备一个被依赖的 lib cone（用于打成 `.cone`）。
        let lib = dir.path().join("lib");
        let lib_src = lib.join("src");
        std::fs::create_dir_all(&lib_src).unwrap();
        std::fs::write(
            lib.join("Cone.toml"),
            r#"
[cone]
name = "fixture-lib"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            lib_src.join("api.scoop"),
            r#"
package fixtures.t1107.lib

import scoop.core.*

public struct Token(val value: Int)
"#,
        )
        .unwrap();
        // 说明：cone source package 约定必须存在 `src/main.scoop`（即使它只是库）。
        std::fs::write(lib_src.join("main.scoop"), "package fixtures.t1107.lib\n").unwrap();

        // 2) 准备一个 consumer app cone：依赖 `fixture-lib`，并在类型层引用 Token。
        let app = dir.path().join("app");
        let app_src = app.join("src");
        let app_cone = app.join("cone");
        std::fs::create_dir_all(&app_src).unwrap();
        std::fs::create_dir_all(&app_cone).unwrap();
        std::fs::write(
            app.join("Cone.toml"),
            r#"
[cone]
name = "fixture-app"
version = "0.0.0"

[dependencies]
fixture-lib = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            app_src.join("main.scoop"),
            r#"
package fixtures.t1107.app

import scoop.core.*
import fixtures.t1107.lib.*

public fun unused(x: Token): Int / Pure! {
    1
}

public fun main() / Pure! {
    println("ok")
}
"#,
        )
        .unwrap();

        // 3) 把 lib 打成 `.cone` 放到 `app/cone/`，让 build 在默认搜索路径下可找到。
        let session = scoopc::session::Session::new().unwrap();
        let pkg = scoopc::cone::load_cone_source_package(&lib).unwrap();
        let out_cone = app_cone.join("fixture-lib-0.0.0.cone");
        scoopc::cone::write_cone_archive_v0(&session, &pkg, &out_cone).unwrap();

        let out = dir.path().join("out").join("a");
        super::run(app, Some(out), super::BuildOptions::default()).unwrap();
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn build_produces_executable_and_it_runs() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(input, Some(out.clone()), super::BuildOptions::default()).unwrap();
        assert!(out.is_file(), "build 应写出可执行文件");

        let status = std::process::Command::new(&out).status().unwrap();
        assert!(status.success(), "可执行文件应返回 0");
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn build_cone_package_with_cone_deps_produces_exe_and_stdout_ok() {
        let dir = tempdir().unwrap();

        let lib = dir.path().join("lib");
        let lib_src = lib.join("src");
        std::fs::create_dir_all(&lib_src).unwrap();
        std::fs::write(
            lib.join("Cone.toml"),
            r#"
[cone]
name = "fixture-lib"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            lib_src.join("api.scoop"),
            r#"
package fixtures.t1107.lib

import scoop.core.*

public struct Token(val value: Int)
"#,
        )
        .unwrap();
        std::fs::write(lib_src.join("main.scoop"), "package fixtures.t1107.lib\n").unwrap();

        let app = dir.path().join("app");
        let app_src = app.join("src");
        let app_cone = app.join("cone");
        std::fs::create_dir_all(&app_src).unwrap();
        std::fs::create_dir_all(&app_cone).unwrap();
        std::fs::write(
            app.join("Cone.toml"),
            r#"
[cone]
name = "fixture-app"
version = "0.0.0"

[dependencies]
fixture-lib = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            app_src.join("main.scoop"),
            r#"
package fixtures.t1107.app

import scoop.core.*
import fixtures.t1107.lib.*

public fun unused(x: Token): Int / Pure! {
    1
}

public fun main() / Pure! {
    println("ok")
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let pkg = scoopc::cone::load_cone_source_package(&lib).unwrap();
        let out_cone = app_cone.join("fixture-lib-0.0.0.cone");
        scoopc::cone::write_cone_archive_v0(&session, &pkg, &out_cone).unwrap();

        let out = dir.path().join("out").join("a");
        super::run(app, Some(out.clone()), super::BuildOptions::default()).unwrap();
        assert!(out.is_file(), "build 应写出可执行文件");

        let output = std::process::Command::new(&out).output().unwrap();
        assert!(output.status.success(), "可执行文件应返回 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "ok\n");
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_emit_llvm_writes_ll_file() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("main.ll");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(
            input,
            Some(out.clone()),
            super::BuildOptions {
                emit: super::BuildEmit::LlvmIr,
                entry_package: None,
            },
        )
        .unwrap();

        let ll = std::fs::read_to_string(&out).unwrap();
        assert!(ll.contains("define i32 @main("), "应输出 LLVM IR");
    }
}
