//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前支持：
//! - `scoopc build-single-cone --cone-root <dir> --out <dir> ...`：scoop 的 cone DAG
//!   scheduler 派发的子进程入口（P10-T06）。
//! - `scoopc link-cone ...`：scoop 的 facade driver 派发的 link 子进程入口（P10-T06-b）。

use miette::{Context as _, IntoDiagnostic as _, Result};

use scoopc::driver_cli::{
    BuildSingleConeCli, CheckSourceCli, CompilerCli, DumpCli, DumpRttiCli, DumpStackmapsCli,
    EmitArtifactCli, LinkConeCli, USAGE, parse_args,
};

fn main() -> Result<()> {
    let Some(cli) = parse_args(std::env::args().skip(1))? else {
        eprintln!("{USAGE}");
        return Ok(());
    };

    match cli {
        CompilerCli::BuildSingleCone(sub) => run_build_single_cone(sub),
        CompilerCli::LinkCone(sub) => run_link_cone(sub),
        CompilerCli::CheckSource(sub) => run_check_source(sub),
        CompilerCli::EmitArtifact(sub) => run_emit_artifact(sub),
        CompilerCli::Dump(sub) => run_dump(sub),
        CompilerCli::DumpRtti(sub) => run_dump_rtti(sub),
        CompilerCli::DumpStackmaps(sub) => run_dump_stackmaps(sub),
    }
}

fn run_check_source(cli: CheckSourceCli) -> Result<()> {
    scoopc::tool_commands::run_check_source(
        cli.input,
        cli.source,
        cli.phase,
        cli.target_platform,
        cli.session_options.with_env_fallback(),
    )
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

    let session_options = cli.session_options.clone().with_env_fallback();
    let session = scoopc::session::Session::with_options(session_options.clone())?;
    let warning_capture = scoopc::warnings::begin_capture();
    scoopc::single_cone::run_single_cone_artifact_compile(
        &session,
        &cone_root,
        &output_dir,
        cli.inputs_fingerprint,
        &upstream_artifact_dirs,
        &session_options,
        cli.opt_level,
    )?;
    emit_warnings(warning_capture.finish());

    Ok(())
}

fn emit_warnings(warnings: Vec<scoopc::warnings::CompileWarning>) {
    for warning in warnings {
        let (line, col) = scoopc::source::SourceFile::load(warning.file())
            .ok()
            .and_then(|source| source.offset_to_line_col(warning.span().start).ok())
            .unwrap_or((1, 1));
        eprintln!(
            "{}:{line}:{col}: {}",
            warning.file().display(),
            warning.render()
        );
    }
}

fn run_emit_artifact(cli: EmitArtifactCli) -> Result<()> {
    scoopc::tool_commands::run_emit_artifact(
        cli.input,
        cli.output,
        cli.kind,
        cli.opt_level,
        cli.session_options.with_env_fallback(),
    )
}

fn run_dump(cli: DumpCli) -> Result<()> {
    match cli {
        DumpCli::Ast {
            input,
            session_options,
        } => scoopc::tool_commands::run_dump_ast(input, session_options.with_env_fallback()),
        DumpCli::Hir {
            input,
            session_options,
        } => scoopc::tool_commands::run_dump_hir(input, session_options.with_env_fallback()),
        DumpCli::Mir {
            input,
            session_options,
        } => scoopc::tool_commands::run_dump_mir(input, session_options.with_env_fallback()),
        DumpCli::Ir {
            input,
            session_options,
        } => scoopc::tool_commands::run_dump_ir(input, session_options.with_env_fallback()),
        DumpCli::EffectFacts {
            input,
            session_options,
        } => {
            scoopc::tool_commands::run_dump_effect_facts(input, session_options.with_env_fallback())
        }
        DumpCli::EffectLowered {
            input,
            session_options,
        } => scoopc::tool_commands::run_dump_effect_lowered(
            input,
            session_options.with_env_fallback(),
        ),
    }
}

fn run_dump_rtti(cli: DumpRttiCli) -> Result<()> {
    scoopc::tool_commands::run_dump_rtti(
        cli.input,
        cli.type_name,
        cli.session_options.with_env_fallback(),
    )
}

fn run_dump_stackmaps(cli: DumpStackmapsCli) -> Result<()> {
    scoopc::tool_commands::run_dump_stackmaps(cli.input, cli.verify_roots, cli.dump_records)
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
