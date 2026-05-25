//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前支持：
//! - `scoopc <input.scoop> [-o <out.ll>]`：经 LLVM stage 生成 LLVM IR。
//! - `scoopc --obj <input.scoop> [-o <out.o>]`：经 LLVM stage 生成 object 文件。
//! - `scoopc build-single-cone --cone-root <dir> --out <dir> ...`：scoop 的 cone DAG
//!   scheduler 派发的子进程入口（P10-T06）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

use scoopc::driver_cli::{
    BuildSingleConeCli, CompilerCli, EmitMode, LegacyCompilerCli, USAGE, parse_args,
};

fn main() -> Result<()> {
    let Some(cli) = parse_args(std::env::args().skip(1))? else {
        eprintln!("{USAGE}");
        return Ok(());
    };

    match cli {
        CompilerCli::Legacy(legacy) => run_legacy(legacy),
        CompilerCli::BuildSingleCone(sub) => run_build_single_cone(sub),
    }
}

fn run_legacy(cli: LegacyCompilerCli) -> Result<()> {
    let input = cli
        .input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = cli.output.unwrap_or_else(|| match cli.emit_mode {
        EmitMode::LlvmIr => default_ll_path(&input),
        EmitMode::Object => default_obj_path(&input),
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("无法创建输出目录")?;
    }

    let source = scoopc::source::SourceFile::load(&input)?;
    let session = scoopc::session::Session::with_options(cli.session_options)?;

    #[cfg(feature = "llvm")]
    {
        match cli.emit_mode {
            EmitMode::LlvmIr => {
                scoopc::pipeline::emit_virtual_cone_llvm_artifact_to_file(
                    &session,
                    &source,
                    &output,
                    scoopc::pipeline::LlvmArtifactKind::LlvmIr,
                )?;
                eprintln!("已写入 LLVM IR：{}", output.display());
            }
            EmitMode::Object => {
                scoopc::pipeline::emit_virtual_cone_llvm_artifact_to_file(
                    &session,
                    &source,
                    &output,
                    scoopc::pipeline::LlvmArtifactKind::Object,
                )?;
                eprintln!("已写入 object 文件：{}", output.display());
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "llvm"))]
    {
        let _ = &session;
        let _ = &source;
        let _ = &output;
        let subcommand = match cli.emit_mode {
            EmitMode::LlvmIr => "<file>",
            EmitMode::Object => "--obj",
        };
        Err(miette::miette!(
            "`scoopc {subcommand}` 需要启用 LLVM 后端：若你用了 `--no-default-features`，去掉它或加上 `--features llvm`"
        ))
    }
}

fn run_build_single_cone(cli: BuildSingleConeCli) -> Result<()> {
    let cone_root = cli
        .cone_root
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "build-single-cone 无法定位 cone 根目录：{}",
                cli.cone_root.display()
            )
        })?;
    std::fs::create_dir_all(&cli.output_dir)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "build-single-cone 无法创建输出目录：{}",
                cli.output_dir.display()
            )
        })?;
    let output_dir = cli
        .output_dir
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "build-single-cone 输出目录无法 canonicalize：{}",
                cli.output_dir.display()
            )
        })?;

    let mut upstream_artifact_dirs = Vec::with_capacity(cli.upstream_artifact_dirs.len());
    for raw in &cli.upstream_artifact_dirs {
        let dir = raw.canonicalize().into_diagnostic().wrap_err_with(|| {
            format!(
                "build-single-cone 无法定位上游 artifact 目录：{}",
                raw.display()
            )
        })?;
        upstream_artifact_dirs.push(dir);
    }

    let session = scoopc::session::Session::with_options(cli.session_options.clone())?;
    scoopc::single_cone::run_single_cone_artifact_compile(
        &session,
        &cone_root,
        &output_dir,
        cli.inputs_fingerprint,
        &upstream_artifact_dirs,
        &cli.session_options,
    )?;

    if let Some(expected) = cli.expected_cone_id.as_deref() {
        eprintln!(
            "已写入 cone artifact：{} (cone-id={})",
            output_dir.display(),
            expected
        );
    } else {
        eprintln!("已写入 cone artifact：{}", output_dir.display());
    }

    Ok(())
}

fn default_ll_path(input: &Path) -> PathBuf {
    let mut out = input.to_path_buf();
    out.set_extension("ll");
    out
}

fn default_obj_path(input: &Path) -> PathBuf {
    let mut out = input.to_path_buf();
    out.set_extension("o");
    out
}
