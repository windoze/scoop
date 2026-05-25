//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前支持：
//! - `scoopc <input.scoop> [-o <out.ll>]`：经 LLVM stage 生成 LLVM IR。
//! - `scoopc --obj <input.scoop> [-o <out.o>]`：经 LLVM stage 生成 object 文件。
//! - `scoopc build-single-cone --cone-root <dir> --out <dir> ...`：scoop 的 cone DAG
//!   scheduler 派发的子进程入口（P10-T06）。
//! - `scoopc link-cone ...`：scoop 的 facade driver 派发的 link 子进程入口（P10-T06-b）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

use scoopc::driver_cli::{
    BuildSingleConeCli, CompilerCli, EmitMode, LegacyCompilerCli, LinkConeCli, USAGE, parse_args,
};

fn main() -> Result<()> {
    let Some(cli) = parse_args(std::env::args().skip(1))? else {
        eprintln!("{USAGE}");
        return Ok(());
    };

    match cli {
        CompilerCli::Legacy(legacy) => run_legacy(legacy),
        CompilerCli::BuildSingleCone(sub) => run_build_single_cone(sub),
        CompilerCli::LinkCone(sub) => run_link_cone(sub),
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

fn run_link_cone(cli: LinkConeCli) -> Result<()> {
    let consumer_obj = cli
        .consumer_obj
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "link-cone 无法定位 consumer object：{}",
                cli.consumer_obj.display()
            )
        })?;

    let mut dep_objs = Vec::with_capacity(cli.dep_objs.len());
    for raw in &cli.dep_objs {
        dep_objs.push(raw.canonicalize().into_diagnostic().wrap_err_with(|| {
            format!("link-cone 无法定位 dependency object：{}", raw.display())
        })?);
    }
    std::fs::create_dir_all(&cli.output_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("link-cone 无法创建输出目录：{}", cli.output_dir.display()))?;
    std::fs::create_dir_all(&cli.runtime_artifact_dir)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "link-cone 无法创建 runtime artifact 目录：{}",
                cli.runtime_artifact_dir.display()
            )
        })?;

    let output_dir = cli
        .output_dir
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "link-cone 输出目录无法 canonicalize：{}",
                cli.output_dir.display()
            )
        })?;
    let runtime_artifact_dir = cli
        .runtime_artifact_dir
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "link-cone runtime artifact 目录无法 canonicalize：{}",
                cli.runtime_artifact_dir.display()
            )
        })?;
    let expected_cone_id_for_name = cli.expected_cone_id.clone();
    let binary_path = cli.binary_output.unwrap_or_else(|| {
        let name = expected_cone_id_for_name
            .as_deref()
            .map(sanitize_file_name)
            .unwrap_or_else(default_exe_name);
        output_dir.join(name)
    });
    if let Some(parent) = binary_path.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("link-cone 无法创建 binary 输出目录：{}", parent.display())
            })?;
    }

    let response = scoopld::link(scoopld::LinkRequest {
        kind: cli.kind,
        consumer_obj,
        dep_objs,
        runtime_artifact_dir,
        output_dir,
        binary_path,
        extern_libs: cli.extern_libs,
        link_flags: cli.link_flags,
        linker: cli.linker,
        parent_inputs_fingerprint: cli.inputs_fingerprint,
        cone_id: cli.expected_cone_id,
    })?;

    if response.cache_hit {
        eprintln!(
            "link-cone cache hit：{} ({})",
            response.binary_path.display(),
            response.fingerprint_hex
        );
    } else {
        eprintln!(
            "已写入 link binary：{} ({})",
            response.binary_path.display(),
            response.fingerprint_hex
        );
    }
    Ok(())
}

fn default_exe_name() -> String {
    if std::env::consts::EXE_EXTENSION.is_empty() {
        "a.out".to_string()
    } else {
        format!("a.{}", std::env::consts::EXE_EXTENSION)
    }
}

fn sanitize_file_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        default_exe_name()
    } else {
        out
    }
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
