//! `scoop build` 子命令。
//!
//! T0805：实现“前端检查 + 输出路径准备”。
//!
//! T0806：在启用 `scoop` 的 `llvm` feature 时，额外执行：
//! - 生成最小 object（当前阶段仍是固定 `main → ret 0`）；
//! - 调用 clang 链接 object + 早期 C runtime，产出可执行文件。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

#[derive(Debug)]
struct BuildInput {
    /// 当前编译单元的全部源文件（单文件模式为 1 个；cone 包为 `src/**/*.scoop`）。
    sources: Vec<scoopc::source::SourceFile>,
    /// 可执行入口（`main.scoop`）在 `sources` 中的下标。
    main_index: usize,
}

impl BuildInput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        &self.sources[self.main_index]
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
    run_frontend(&session, &input.sources)?;
    // 非 llvm 构建下，codegen 分支会被编译掉；这里显式访问一次 main 以避免 dead_code 警告，
    // 同时也作为“加载逻辑能稳定定位入口”的最小一致性校验。
    let _ = input.main_source();

    match options.emit {
        BuildEmit::Executable => {
            // 只有在启用 LLVM 后端时才会真正生成可执行文件；默认构建仍保持“前端检查”可用。
            #[cfg(feature = "llvm")]
            run_codegen_and_link(&session, input.main_source(), &output)?;
        }
        BuildEmit::LlvmIr => {
            #[cfg(feature = "llvm")]
            {
                scoopc::llvm::emit_minimal_main_ir_to_file(&session, input.main_source(), &output)?;
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
                scoopc::llvm::emit_minimal_main_obj_to_file(&session, input.main_source(), &output)?;
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
                scoopc::llvm::emit_minimal_main_asm_to_file(&session, input.main_source(), &output)?;
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

        return Ok(BuildInput { sources, main_index });
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

fn run_frontend(session: &scoopc::session::Session, sources: &[scoopc::source::SourceFile]) -> Result<()> {
    if sources.is_empty() {
        return Err(miette::miette!("内部错误：build 输入 sources 为空"));
    }

    // 先 parse 所有文件（cone 包模式下：`src/**/*.scoop`）。
    let mut asts = Vec::with_capacity(sources.len());
    for source in sources {
        let ast = scoopc::parser::parse_file(source).map_err(miette::Report::from)?;
        asts.push(ast);
    }

    // 先运行不依赖 resolver/index 的 typecheck 预检查（与 fixtures/typecheck pipeline 对齐）。
    for (source, ast) in sources.iter().zip(asts.iter()) {
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
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(1),
            source,
            file: ast,
        });
    }

    let index = scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // resolver phase：headers + bodies（逐文件运行，但共享同一个 index）。
    let mut headers = Vec::with_capacity(sources.len());
    for (source, ast) in sources.iter().zip(asts.iter()) {
        let h = scoopc::resolve::check_file_headers(source, ast, &index).map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in sources.iter().zip(asts.iter_mut()).zip(headers.iter()) {
        scoopc::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    // type env：sysroot + 当前 cone 全部文件（用于跨文件 TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    // typecheck phase：逐文件执行（共享 env/index/types）。
    for ((source, ast), h) in sources.iter().zip(asts.iter()).zip(headers.iter()) {
        scoopc::typecheck::check_file_annotations(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_properties(source, ast, &index, &env).map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_inheritance(source, ast, &index).map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_interfaces(source, ast, &index, &env).map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_override_effects(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_type_refs(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_where_clauses(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_overload_conflicts(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_exprs(
            source,
            ast,
            &index,
            &h.imports,
            &env,
            &mut types,
            builtins,
        )
        .map_err(miette::Report::from)?;
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 fixtures/typecheck_multi 对齐）。
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
