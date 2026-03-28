//! `scoop build` 子命令。
//!
//! T0805：实现“前端检查 + 输出路径准备”。
//!
//! T0806：在启用 `scoop` 的 `llvm` feature 时，额外执行：
//! - 生成最小 object（当前阶段仍是固定 `main → ret 0`）；
//! - 调用 clang 链接 object + 早期 C runtime，产出可执行文件。

mod deps;

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

#[derive(Debug)]
struct BuildInput {
    /// 当前编译单元的全部源文件（单文件模式为 1 个；cone 包为 `src/**/*.scoop`）。
    sources: Vec<scoopc::source::SourceFile>,
    /// 可执行入口（`main.scoop`）在 `sources` 中的下标。
    main_index: usize,
    /// 若输入为 cone 包目录，则包含其 root 与 manifest（用于 T1107 依赖图解析）。
    cone_root: Option<PathBuf>,
    cone_manifest: Option<scoopc::cone::ConeManifest>,
}

impl BuildInput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        &self.sources[self.main_index]
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

#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    pub emit: BuildEmit,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            emit: BuildEmit::Executable,
        }
    }
}

/// 执行 `scoop build <input> [-o <output>]`。
///
/// 当前阶段验收点：
/// - 输入可通过 parse/resolve/typecheck 时返回 `Ok(())`；
/// - 当启用 `--features llvm` 时：
///   - 默认产出可执行文件；
///   - 若指定 `--emit-llvm/--emit-obj/--emit-asm`，则改为产出对应单文件产物。
pub fn run(input: PathBuf, output: Option<PathBuf>, options: BuildOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = output.unwrap_or_else(|| default_output_path_for_emit(options.emit));
    ensure_output_parent_dir(&output)?;

    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    let session = scoopc::session::Session::new()?;

    let input = load_build_input(&input)?;
    let deps = match (&input.cone_root, &input.cone_manifest) {
        (Some(root), Some(manifest)) => deps::load_dependency_graph(manifest, root)?,
        _ => Vec::new(),
    };
    let front = run_frontend(&session, input, &deps)?;
    // 非 llvm 构建下，codegen 分支会被编译掉；这里显式访问一次 main 以避免 dead_code 警告，
    // 同时也作为“加载逻辑能稳定定位入口”的最小一致性校验。
    let _ = front.main_source();

    match options.emit {
        BuildEmit::Executable => {
            // 只有在启用 LLVM 后端时才会真正生成可执行文件；默认构建仍保持“前端检查”可用。
            #[cfg(feature = "llvm")]
            run_codegen_and_link(&session, &front, &output)?;
        }
        BuildEmit::LlvmIr => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_ir_to_file_from_lowered_hir(
                    front.main_source(),
                    &lowered,
                    &output,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-llvm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop --features llvm -- build --emit-llvm <file> -o <out.ll>`"
                ));
            }
        }
        BuildEmit::Obj => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir(
                    front.main_source(),
                    &lowered,
                    &output,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-obj` 需要启用 LLVM 后端：请使用 `cargo run -p scoop --features llvm -- build --emit-obj <file> -o <out.o>`"
                ));
            }
        }
        BuildEmit::Asm => {
            #[cfg(feature = "llvm")]
            {
                let lowered = lower_main_hir_for_build(&session, &front)?;
                scoopc::llvm::emit_minimal_main_asm_to_file_from_lowered_hir(
                    front.main_source(),
                    &lowered,
                    &output,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-asm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop --features llvm -- build --emit-asm <file> -o <out.s>`"
                ));
            }
        }
    }

    Ok(())
}

fn load_build_input(input: &Path) -> Result<BuildInput> {
    // 单文件模式：保持 `scoop build <file.scoop>` 的原有行为。
    if input.is_file() {
        return Ok(BuildInput {
            sources: vec![scoopc::source::SourceFile::load(input)?],
            main_index: 0,
            cone_root: None,
            cone_manifest: None,
        });
    }

    // cone 包模式：`scoop build <cone-root>`（按 T1102 规则定位 `src/main.scoop`）。
    if input.is_dir() {
        let pkg = scoopc::cone::load_cone_source_package(input)?;
        let mut sources = Vec::with_capacity(pkg.sources.len());
        let mut main_index = None;
        for (idx, path) in pkg.sources.iter().enumerate() {
            let source = scoopc::source::SourceFile::load(path)?;
            if source.path() == pkg.main.as_path() {
                main_index = Some(idx);
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
            cone_root: Some(pkg.root),
            cone_manifest: Some(pkg.manifest),
        });
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

fn run_frontend(
    session: &scoopc::session::Session,
    input: BuildInput,
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
    scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir(
        front.main_source(),
        &lowered,
        &obj,
    )?;
    crate::toolchain::link_obj_with_runtime(&obj, output)?;

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

    scoopc::hir::lower_for_compilation_unit(
        front.main_source(),
        front.main_ast(),
        &front.index,
        &unit,
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
            },
        )
        .unwrap();

        let ll = std::fs::read_to_string(&out).unwrap();
        assert!(ll.contains("define i32 @main()"), "应输出 LLVM IR");
    }
}
