//! `scoopc` CLI 参数解析。
//!
//! 该模块只负责把原始参数解析成稳定结构，避免把 session 配置散落在二进制入口里。

use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;

use miette::Result;

use crate::cone::ConeKind;
use crate::opt::OptLevel;
use crate::session::SessionOptions;
use crate::tool_commands::{CheckSourcePhase, EmitArtifactKind};

pub const USAGE: &str = "\
用法：
  scoopc build-single-cone --cone-root <dir> --out <dir> \\
      --inputs-fingerprint <hex> [--upstream-artifact <dir> ...] [--cone-id <key>] \\
      [--opt-level <0|1|2|3|s|z>] [--sysroot-dep <name> ...]
  scoopc link-cone --kind <bin|lib|syslib> --consumer-obj <path> \\
      [--dep-obj <path> ...] --runtime-artifact-dir <dir> --out <dir> \\
      --inputs-fingerprint <hex> [--binary-out <path>] [--linker <path>] \\
      [--extern-lib <name> ...] [--link-flag <flag> ...] [--cone-id <key>]
  scoopc emit-artifact --kind <llvm-ir|obj|asm> --input <path> --out <path> \
      [--opt-level <0|1|2|3|s|z>]
  scoopc check-source --phase <parse|resolve|typecheck|infer> --input <file-or-cone-dir> \
      [--source <path>] [--target-platform <id>]
  scoopc dump-ast|dump-hir|dump-mir|dump-ir|dump-effect-facts|dump-effect-lowered <path>
  scoopc dump-rtti <path> [--type <TYPE>]
  scoopc dump-stackmaps [--verify-roots] [--dump-records] <path>
  scoopc test-fixtures [--fixtures <path>] [--processes <N>] [--exit-on-failure] \
      [--opt-level <0|1|2|3|s|z>] [--gc-stress] [--gc-move] [--threads <N>]

说明：
  - 该二进制需要启用 `scoopc` 的 `llvm` feature（需要 LLVM 21.1 + `llvm-config`）。
  - `build-single-cone` 由 scoop 的 cone DAG scheduler 通过子进程派发，将 cone-being
    -compiled 当作 graph consumer 跑完整 frontend 并写入 per-cone artifact。
";

/// scoopc CLI 顶层模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerCli {
    /// `scoopc build-single-cone ...`：scoop driver 派发的子进程入口。
    BuildSingleCone(BuildSingleConeCli),
    /// `scoopc link-cone ...`：scoop driver 派发的 link 子进程入口。
    LinkCone(LinkConeCli),
    CheckSource(CheckSourceCli),
    EmitArtifact(EmitArtifactCli),
    Dump(DumpCli),
    DumpRtti(DumpRttiCli),
    DumpStackmaps(DumpStackmapsCli),
    TestFixtures(crate::fixture_cli::FixtureCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSourceCli {
    pub phase: CheckSourcePhase,
    pub input: PathBuf,
    pub source: Option<PathBuf>,
    pub target_platform: Option<String>,
    pub session_options: SessionOptions,
}

/// `scoopc build-single-cone` 子命令的参数。
///
/// 该子命令仅作为 scoop driver -> scoopc 子进程的稳定接口；裸调用应优先经由 scoop
/// driver。CLI 形态与 [`crate::single_cone::run_single_cone_artifact_compile`] 字段一一对应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSingleConeCli {
    /// cone-being-compiled 的根目录（含 `Cone.toml`）。
    pub cone_root: PathBuf,
    /// per-cone artifact 输出目录（写 `manifest.json` / `*.bin` / `inputs.fingerprint` ...）。
    pub output_dir: PathBuf,
    /// 父进程为本 cone 计算的 inputs fingerprint，hex 编码。
    pub inputs_fingerprint: Vec<u8>,
    /// 已构建好的上游 cone artifact 目录列表（顺序无关；按 manifest 中 StableConeKey 匹配）。
    pub upstream_artifact_dirs: Vec<PathBuf>,
    /// 期望的 cone stable key（"name@version"）。可选，仅作为父子进程一致性校验。
    pub expected_cone_id: Option<String>,
    /// 子进程内 LLVM/LIR pipeline 使用的优化等级。
    pub opt_level: OptLevel,
    /// 子进程内的 session 选项（profile / sysroot overlay / extra sysroot deps 等）。
    pub session_options: SessionOptions,
}

/// `scoopc link-cone` 子命令的参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkConeCli {
    pub kind: ConeKind,
    pub consumer_obj: PathBuf,
    pub dep_objs: Vec<PathBuf>,
    pub runtime_artifact_dir: PathBuf,
    pub output_dir: PathBuf,
    pub binary_output: Option<PathBuf>,
    pub extern_libs: Vec<String>,
    pub link_flags: Vec<String>,
    pub linker: Option<PathBuf>,
    pub inputs_fingerprint: Vec<u8>,
    pub expected_cone_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitArtifactCli {
    pub kind: EmitArtifactKind,
    pub input: PathBuf,
    pub output: PathBuf,
    pub opt_level: OptLevel,
    pub session_options: SessionOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DumpCli {
    Ast {
        input: PathBuf,
        session_options: SessionOptions,
    },
    Hir {
        input: PathBuf,
        session_options: SessionOptions,
    },
    Mir {
        input: PathBuf,
        session_options: SessionOptions,
    },
    Ir {
        input: PathBuf,
        session_options: SessionOptions,
    },
    EffectFacts {
        input: PathBuf,
        session_options: SessionOptions,
    },
    EffectLowered {
        input: PathBuf,
        session_options: SessionOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpRttiCli {
    pub input: PathBuf,
    pub type_name: Option<String>,
    pub session_options: SessionOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpStackmapsCli {
    pub input: PathBuf,
    pub verify_roots: bool,
    pub dump_records: bool,
}

pub fn parse_args<I, S>(args: I) -> Result<Option<CompilerCli>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args_iter = args.into_iter().map(Into::into);
    let first = args_iter.next();
    match first.as_deref() {
        Some("-h") | Some("--help") => Ok(None),
        Some("build-single-cone") => {
            parse_build_single_cone(args_iter).map(|cli| Some(CompilerCli::BuildSingleCone(cli)))
        }
        Some("link-cone") => parse_link_cone(args_iter).map(|cli| Some(CompilerCli::LinkCone(cli))),
        Some("check-source") => {
            parse_check_source(args_iter).map(|cli| Some(CompilerCli::CheckSource(cli)))
        }
        Some("emit-artifact") => {
            parse_emit_artifact(args_iter).map(|cli| Some(CompilerCli::EmitArtifact(cli)))
        }
        Some("dump-ast") => parse_dump_input(args_iter).map(|(input, session_options)| {
            Some(CompilerCli::Dump(DumpCli::Ast {
                input,
                session_options,
            }))
        }),
        Some("dump-hir") => parse_dump_input(args_iter).map(|(input, session_options)| {
            Some(CompilerCli::Dump(DumpCli::Hir {
                input,
                session_options,
            }))
        }),
        Some("dump-mir") => parse_dump_input(args_iter).map(|(input, session_options)| {
            Some(CompilerCli::Dump(DumpCli::Mir {
                input,
                session_options,
            }))
        }),
        Some("dump-ir") => parse_dump_input(args_iter).map(|(input, session_options)| {
            Some(CompilerCli::Dump(DumpCli::Ir {
                input,
                session_options,
            }))
        }),
        Some("dump-effect-facts") => parse_dump_input(args_iter).map(|(input, session_options)| {
            Some(CompilerCli::Dump(DumpCli::EffectFacts {
                input,
                session_options,
            }))
        }),
        Some("dump-effect-lowered") => {
            parse_dump_input(args_iter).map(|(input, session_options)| {
                Some(CompilerCli::Dump(DumpCli::EffectLowered {
                    input,
                    session_options,
                }))
            })
        }
        Some("dump-rtti") => parse_dump_rtti(args_iter).map(|cli| Some(CompilerCli::DumpRtti(cli))),
        Some("dump-stackmaps") => {
            parse_dump_stackmaps(args_iter).map(|cli| Some(CompilerCli::DumpStackmaps(cli)))
        }
        Some("test-fixtures") => {
            parse_test_fixtures(args_iter).map(|cli| Some(CompilerCli::TestFixtures(cli)))
        }
        Some(other) => Err(miette::miette!(
            "未知 scoopc 子命令或参数：{other}\n\n{USAGE}"
        )),
        None => Err(miette::miette!("缺少 scoopc 子命令\n\n{USAGE}")),
    }
}

fn parse_build_single_cone<I>(args: I) -> Result<BuildSingleConeCli>
where
    I: IntoIterator<Item = String>,
{
    let mut cone_root: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut inputs_fingerprint_hex: Option<String> = None;
    let mut upstream_artifact_dirs: Vec<PathBuf> = Vec::new();
    let mut expected_cone_id: Option<String> = None;
    let mut opt_level: Option<OptLevel> = None;
    let mut extra_sysroot_deps: Vec<String> = Vec::new();

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Err(miette::miette!("{USAGE}"));
            }
            "--cone-root" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`--cone-root` 需要一个目录路径\n\n{USAGE}"));
                };
                cone_root = Some(PathBuf::from(value));
            }
            "--out" | "--output-dir" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`{arg}` 需要一个目录路径\n\n{USAGE}"));
                };
                output_dir = Some(PathBuf::from(value));
            }
            "--inputs-fingerprint" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--inputs-fingerprint` 需要一个 hex 字符串\n\n{USAGE}"
                    ));
                };
                inputs_fingerprint_hex = Some(value);
            }
            "--upstream-artifact" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--upstream-artifact` 需要一个目录路径\n\n{USAGE}"
                    ));
                };
                upstream_artifact_dirs.push(PathBuf::from(value));
            }
            "--cone-id" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--cone-id` 需要一个 cone stable-key 字符串\n\n{USAGE}"
                    ));
                };
                expected_cone_id = Some(value);
            }
            "--opt-level" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`--opt-level` 需要一个优化等级\n\n{USAGE}"));
                };
                opt_level = Some(OptLevel::parse(&value)?);
            }
            "--sysroot-dep" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--sysroot-dep` 需要一个 sysroot cone 名称\n\n{USAGE}"
                    ));
                };
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(miette::miette!("`--sysroot-dep` 不能是空字符串"));
                }
                extra_sysroot_deps.push(trimmed.to_string());
            }
            _ => {
                return Err(miette::miette!(
                    "build-single-cone 不接受参数 `{arg}`\n\n{USAGE}"
                ));
            }
        }
    }

    let cone_root = cone_root
        .ok_or_else(|| miette::miette!("`build-single-cone` 缺少 `--cone-root`\n\n{USAGE}"))?;
    let output_dir =
        output_dir.ok_or_else(|| miette::miette!("`build-single-cone` 缺少 `--out`\n\n{USAGE}"))?;
    let hex_value = inputs_fingerprint_hex.ok_or_else(|| {
        miette::miette!("`build-single-cone` 缺少 `--inputs-fingerprint`\n\n{USAGE}")
    })?;
    let inputs_fingerprint = decode_hex(&hex_value).ok_or_else(|| {
        miette::miette!("`--inputs-fingerprint` 必须是偶数长度的 hex 字符串：{hex_value}")
    })?;

    let session_options = SessionOptions::new().with_extra_sysroot_dependencies(extra_sysroot_deps);

    Ok(BuildSingleConeCli {
        cone_root,
        output_dir,
        inputs_fingerprint,
        upstream_artifact_dirs,
        expected_cone_id,
        opt_level: opt_level.unwrap_or(OptLevel::O0),
        session_options,
    })
}

fn parse_link_cone<I>(args: I) -> Result<LinkConeCli>
where
    I: IntoIterator<Item = String>,
{
    let mut kind: Option<ConeKind> = None;
    let mut consumer_obj: Option<PathBuf> = None;
    let mut dep_objs: Vec<PathBuf> = Vec::new();
    let mut runtime_artifact_dir: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut binary_output: Option<PathBuf> = None;
    let mut extern_libs: Vec<String> = Vec::new();
    let mut link_flags: Vec<String> = Vec::new();
    let mut linker: Option<PathBuf> = None;
    let mut inputs_fingerprint_hex: Option<String> = None;
    let mut expected_cone_id: Option<String> = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(miette::miette!("{USAGE}")),
            "--kind" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--kind` 需要 `bin`、`lib` 或 `syslib`\n\n{USAGE}"
                    ));
                };
                kind = Some(ConeKind::parse(&value)?);
            }
            "--consumer-obj" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--consumer-obj` 需要一个 object 路径\n\n{USAGE}"
                    ));
                };
                consumer_obj = Some(PathBuf::from(value));
            }
            "--dep-obj" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--dep-obj` 需要一个 object 路径\n\n{USAGE}"
                    ));
                };
                dep_objs.push(PathBuf::from(value));
            }
            "--runtime-artifact-dir" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--runtime-artifact-dir` 需要一个目录路径\n\n{USAGE}"
                    ));
                };
                runtime_artifact_dir = Some(PathBuf::from(value));
            }
            "--out" | "--output-dir" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`{arg}` 需要一个目录路径\n\n{USAGE}"));
                };
                output_dir = Some(PathBuf::from(value));
            }
            "--binary-out" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--binary-out` 需要一个输出路径\n\n{USAGE}"
                    ));
                };
                binary_output = Some(PathBuf::from(value));
            }
            "--extern-lib" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`--extern-lib` 需要库名\n\n{USAGE}"));
                };
                extern_libs.push(value);
            }
            "--link-flag" | "--link-flags" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`{arg}` 需要一个 linker flag\n\n{USAGE}"));
                };
                link_flags.push(value);
            }
            "--linker" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--linker` 需要一个链接器路径或名称\n\n{USAGE}"
                    ));
                };
                linker = Some(PathBuf::from(value));
            }
            "--inputs-fingerprint" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--inputs-fingerprint` 需要一个 hex 字符串\n\n{USAGE}"
                    ));
                };
                inputs_fingerprint_hex = Some(value);
            }
            "--cone-id" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--cone-id` 需要一个 cone stable-key 字符串\n\n{USAGE}"
                    ));
                };
                expected_cone_id = Some(value);
            }
            _ => return Err(miette::miette!("link-cone 不接受参数 `{arg}`\n\n{USAGE}")),
        }
    }

    let hex_value = inputs_fingerprint_hex
        .ok_or_else(|| miette::miette!("`link-cone` 缺少 `--inputs-fingerprint`\n\n{USAGE}"))?;
    let inputs_fingerprint = decode_hex(&hex_value).ok_or_else(|| {
        miette::miette!("`--inputs-fingerprint` 必须是偶数长度的 hex 字符串：{hex_value}")
    })?;

    Ok(LinkConeCli {
        kind: kind.ok_or_else(|| miette::miette!("`link-cone` 缺少 `--kind`\n\n{USAGE}"))?,
        consumer_obj: consumer_obj
            .ok_or_else(|| miette::miette!("`link-cone` 缺少 `--consumer-obj`\n\n{USAGE}"))?,
        dep_objs,
        runtime_artifact_dir: runtime_artifact_dir.ok_or_else(|| {
            miette::miette!("`link-cone` 缺少 `--runtime-artifact-dir`\n\n{USAGE}")
        })?,
        output_dir: output_dir
            .ok_or_else(|| miette::miette!("`link-cone` 缺少 `--out`\n\n{USAGE}"))?,
        binary_output,
        extern_libs,
        link_flags,
        linker,
        inputs_fingerprint,
        expected_cone_id,
    })
}

fn parse_check_source<I>(args: I) -> Result<CheckSourceCli>
where
    I: IntoIterator<Item = String>,
{
    let mut phase = None;
    let mut input = None;
    let mut source = None;
    let mut target_platform = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(miette::miette!("{USAGE}")),
            "--phase" => {
                phase = Some(CheckSourcePhase::parse(&required_arg(
                    &mut args, "--phase",
                )?)?)
            }
            "--input" => input = Some(PathBuf::from(required_arg(&mut args, "--input")?)),
            "--source" => source = Some(PathBuf::from(required_arg(&mut args, "--source")?)),
            "--target-platform" => {
                let value = required_arg(&mut args, "--target-platform")?;
                if value.trim().is_empty() {
                    return Err(miette::miette!("`--target-platform` 不能是空字符串"));
                }
                target_platform = Some(value);
            }
            other => {
                return Err(miette::miette!(
                    "check-source 不接受参数 `{other}`\n\n{USAGE}"
                ));
            }
        }
    }
    Ok(CheckSourceCli {
        phase: phase.ok_or_else(|| miette::miette!("check-source 缺少 --phase\n\n{USAGE}"))?,
        input: input.ok_or_else(|| miette::miette!("check-source 缺少 --input\n\n{USAGE}"))?,
        source,
        target_platform,
        session_options: SessionOptions::new(),
    })
}

fn parse_emit_artifact<I>(args: I) -> Result<EmitArtifactCli>
where
    I: IntoIterator<Item = String>,
{
    let mut kind = None;
    let mut input = None;
    let mut output = None;
    let mut opt_level = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--kind" => {
                kind = Some(EmitArtifactKind::parse(&required_arg(
                    &mut args, "--kind",
                )?)?)
            }
            "--input" => input = Some(PathBuf::from(required_arg(&mut args, "--input")?)),
            "--out" | "-o" => output = Some(PathBuf::from(required_arg(&mut args, &arg)?)),
            "--opt-level" => {
                opt_level = Some(OptLevel::parse(&required_arg(&mut args, "--opt-level")?)?)
            }
            other => {
                return Err(miette::miette!(
                    "emit-artifact 不接受参数 `{other}`\n\n{USAGE}"
                ));
            }
        }
    }
    Ok(EmitArtifactCli {
        kind: kind.ok_or_else(|| miette::miette!("emit-artifact 缺少 --kind\n\n{USAGE}"))?,
        input: input.ok_or_else(|| miette::miette!("emit-artifact 缺少 --input\n\n{USAGE}"))?,
        output: output.ok_or_else(|| miette::miette!("emit-artifact 缺少 --out\n\n{USAGE}"))?,
        opt_level: opt_level.unwrap_or(OptLevel::O0),
        session_options: SessionOptions::new(),
    })
}

fn parse_dump_input<I>(args: I) -> Result<(PathBuf, SessionOptions)>
where
    I: IntoIterator<Item = String>,
{
    let mut input = None;
    for arg in args {
        if input.is_none() {
            input = Some(PathBuf::from(arg));
        } else {
            return Err(miette::miette!("dump 子命令只接受一个输入路径\n\n{USAGE}"));
        }
    }
    Ok((
        input.ok_or_else(|| miette::miette!("dump 子命令缺少输入路径\n\n{USAGE}"))?,
        SessionOptions::new(),
    ))
}

fn parse_dump_rtti<I>(args: I) -> Result<DumpRttiCli>
where
    I: IntoIterator<Item = String>,
{
    let mut input = None;
    let mut type_name = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--type" => type_name = Some(required_arg(&mut args, "--type")?),
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return Err(miette::miette!("dump-rtti 不接受参数 `{other}`\n\n{USAGE}")),
        }
    }
    Ok(DumpRttiCli {
        input: input.ok_or_else(|| miette::miette!("dump-rtti 缺少输入路径\n\n{USAGE}"))?,
        type_name,
        session_options: SessionOptions::new(),
    })
}

fn parse_dump_stackmaps<I>(args: I) -> Result<DumpStackmapsCli>
where
    I: IntoIterator<Item = String>,
{
    let mut input = None;
    let mut verify_roots = false;
    let mut dump_records = false;
    for arg in args {
        match arg.as_str() {
            "--verify-roots" => verify_roots = true,
            "--dump-records" => dump_records = true,
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => {
                return Err(miette::miette!(
                    "dump-stackmaps 不接受参数 `{other}`\n\n{USAGE}"
                ));
            }
        }
    }
    Ok(DumpStackmapsCli {
        input: input.ok_or_else(|| miette::miette!("dump-stackmaps 缺少输入路径\n\n{USAGE}"))?,
        verify_roots,
        dump_records,
    })
}

fn parse_test_fixtures<I>(args: I) -> Result<crate::fixture_cli::FixtureCliOptions>
where
    I: IntoIterator<Item = String>,
{
    let mut options = crate::fixture_cli::FixtureCliOptions::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fixtures" => {
                options.fixtures = Some(PathBuf::from(required_arg(&mut args, "--fixtures")?))
            }
            "--processes" => {
                let raw = required_arg(&mut args, "--processes")?;
                let parsed: usize = raw
                    .parse()
                    .map_err(|_| miette::miette!("--processes 必须是正整数：{raw}"))?;
                options.processes = NonZeroUsize::new(parsed)
                    .ok_or_else(|| miette::miette!("--processes 必须大于 0"))?;
            }
            "--exit-on-failure" => options.exit_on_failure = true,
            "--opt-level" => {
                options.opt_level = Some(OptLevel::parse(&required_arg(&mut args, "--opt-level")?)?)
            }
            "--gc-stress" => options.gc_stress = true,
            "--gc-move" => options.gc_move = true,
            "--threads" => {
                let raw = required_arg(&mut args, "--threads")?;
                let parsed: u32 = raw
                    .parse()
                    .map_err(|_| miette::miette!("--threads 必须是正整数：{raw}"))?;
                options.threads = Some(
                    NonZeroU32::new(parsed)
                        .ok_or_else(|| miette::miette!("--threads 必须大于 0"))?,
                );
            }
            other => {
                return Err(miette::miette!(
                    "test-fixtures 不接受参数 `{other}`\n\n{USAGE}"
                ));
            }
        }
    }
    Ok(options)
}

fn required_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| miette::miette!("`{name}` 需要一个参数值\n\n{USAGE}"))
}

fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = decode_nibble(chunk[0])?;
        let lo = decode_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn decode_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(10 + (byte - b'a')),
        b'A'..=b'F' => Some(10 + (byte - b'A')),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn build_single_cone(cli: CompilerCli) -> BuildSingleConeCli {
        match cli {
            CompilerCli::BuildSingleCone(c) => c,
            other => panic!("expected BuildSingleCone, got {other:?}"),
        }
    }

    fn link_cone(cli: CompilerCli) -> LinkConeCli {
        match cli {
            CompilerCli::LinkCone(c) => c,
            other => panic!("expected LinkCone, got {other:?}"),
        }
    }

    fn check_source(cli: CompilerCli) -> CheckSourceCli {
        match cli {
            CompilerCli::CheckSource(c) => c,
            other => panic!("expected CheckSource, got {other:?}"),
        }
    }

    #[test]
    fn bare_file_legacy_cli_is_removed() {
        let err = parse_args(["input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("未知 scoopc 子命令"));
        assert!(!USAGE.contains("--emit-llvm"));
    }

    #[test]
    fn obj_legacy_cli_is_removed() {
        let err = parse_args(["--obj", "input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("未知 scoopc 子命令"));
    }

    #[test]
    fn effect_pipeline_selector_removed_for_scoopc_cli() {
        let err = parse_args(["--effect-pipeline", "legacy", "--obj", "input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("未知 scoopc 子命令"));
    }

    #[test]
    fn parse_args_rejects_removed_effect_pipeline_selector_with_any_value() {
        let err =
            parse_args(["--effect-pipeline", "future", "--emit-llvm", "input.scoop"]).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("未知 scoopc 子命令"));
    }

    #[test]
    fn removed_llvm_and_object_flags_are_unknown() {
        let err = parse_args(["--emit-llvm", "--obj", "input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("未知 scoopc 子命令"));
    }

    #[test]
    fn build_single_cone_minimal_cli_parses() {
        let cli = build_single_cone(
            parse_args([
                "build-single-cone",
                "--cone-root",
                "/tmp/foo",
                "--out",
                "/tmp/bar",
                "--inputs-fingerprint",
                "abcd",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(cli.cone_root, PathBuf::from("/tmp/foo"));
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/bar"));
        assert_eq!(cli.inputs_fingerprint, vec![0xab, 0xcd]);
        assert!(cli.upstream_artifact_dirs.is_empty());
        assert_eq!(cli.expected_cone_id, None);
    }

    #[test]
    fn build_single_cone_collects_repeated_upstream_artifacts() {
        let cli = build_single_cone(
            parse_args([
                "build-single-cone",
                "--cone-root",
                "/tmp/foo",
                "--out",
                "/tmp/bar",
                "--inputs-fingerprint",
                "00",
                "--upstream-artifact",
                "/tmp/up1",
                "--upstream-artifact",
                "/tmp/up2",
                "--cone-id",
                "demo@0.0.0",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(
            cli.upstream_artifact_dirs,
            vec![PathBuf::from("/tmp/up1"), PathBuf::from("/tmp/up2")]
        );
        assert_eq!(cli.expected_cone_id.as_deref(), Some("demo@0.0.0"));
    }

    #[test]
    fn build_single_cone_rejects_odd_length_hex() {
        let err = parse_args([
            "build-single-cone",
            "--cone-root",
            "/tmp/foo",
            "--out",
            "/tmp/bar",
            "--inputs-fingerprint",
            "abc",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("hex"));
    }

    #[test]
    fn build_single_cone_rejects_missing_required_arg() {
        let err = parse_args([
            "build-single-cone",
            "--cone-root",
            "/tmp/foo",
            "--inputs-fingerprint",
            "00",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--out"));
    }

    #[test]
    fn check_source_cli_parses_required_and_optional_args() {
        let cli = check_source(
            parse_args([
                "check-source",
                "--phase",
                "typecheck",
                "--input",
                "/tmp/project",
                "--source",
                "src/main.scoop",
                "--target-platform",
                "wasm32-unknown",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(cli.phase, CheckSourcePhase::Typecheck);
        assert_eq!(cli.input, PathBuf::from("/tmp/project"));
        assert_eq!(cli.source, Some(PathBuf::from("src/main.scoop")));
        assert_eq!(cli.target_platform.as_deref(), Some("wasm32-unknown"));
    }

    #[test]
    fn check_source_cli_rejects_unknown_phase() {
        let err = parse_args([
            "check-source",
            "--phase",
            "lower",
            "--input",
            "/tmp/project",
        ])
        .unwrap_err();

        assert!(err.to_string().contains("parse|resolve|typecheck|infer"));
    }

    #[test]
    fn link_cone_minimal_cli_parses() {
        let cli = link_cone(
            parse_args([
                "link-cone",
                "--kind",
                "bin",
                "--consumer-obj",
                "/tmp/main.o",
                "--runtime-artifact-dir",
                "/tmp/runtime",
                "--out",
                "/tmp/link",
                "--inputs-fingerprint",
                "abcd",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(cli.kind, ConeKind::Bin);
        assert_eq!(cli.consumer_obj, PathBuf::from("/tmp/main.o"));
        assert_eq!(cli.runtime_artifact_dir, PathBuf::from("/tmp/runtime"));
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/link"));
        assert_eq!(cli.inputs_fingerprint, vec![0xab, 0xcd]);
        assert!(cli.dep_objs.is_empty());
    }

    #[test]
    fn link_cone_collects_repeated_values() {
        let cli = link_cone(
            parse_args([
                "link-cone",
                "--kind",
                "bin",
                "--consumer-obj",
                "/tmp/main.o",
                "--dep-obj",
                "/tmp/dep1.o",
                "--dep-obj",
                "/tmp/dep2.o",
                "--runtime-artifact-dir",
                "/tmp/runtime",
                "--out",
                "/tmp/link",
                "--binary-out",
                "/tmp/app",
                "--inputs-fingerprint",
                "00",
                "--extern-lib",
                "m",
                "--link-flag",
                "-Wl,--gc-sections",
                "--linker",
                "clang++",
                "--cone-id",
                "app@0.0.0",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(
            cli.dep_objs,
            vec![PathBuf::from("/tmp/dep1.o"), PathBuf::from("/tmp/dep2.o")]
        );
        assert_eq!(cli.binary_output, Some(PathBuf::from("/tmp/app")));
        assert_eq!(cli.extern_libs, vec!["m".to_string()]);
        assert_eq!(cli.link_flags, vec!["-Wl,--gc-sections".to_string()]);
        assert_eq!(cli.linker, Some(PathBuf::from("clang++")));
        assert_eq!(cli.expected_cone_id.as_deref(), Some("app@0.0.0"));
    }

    #[test]
    fn link_cone_rejects_invalid_kind() {
        let err = parse_args([
            "link-cone",
            "--kind",
            "dylib",
            "--consumer-obj",
            "/tmp/main.o",
            "--runtime-artifact-dir",
            "/tmp/runtime",
            "--out",
            "/tmp/link",
            "--inputs-fingerprint",
            "00",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("bin"));
    }

    #[test]
    fn link_cone_rejects_missing_required_arg() {
        let err = parse_args([
            "link-cone",
            "--kind",
            "bin",
            "--runtime-artifact-dir",
            "/tmp/runtime",
            "--out",
            "/tmp/link",
            "--inputs-fingerprint",
            "00",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--consumer-obj"));
    }

    #[test]
    fn link_cone_rejects_unknown_arg() {
        let err = parse_args([
            "link-cone",
            "--kind",
            "bin",
            "--consumer-obj",
            "/tmp/main.o",
            "--runtime-artifact-dir",
            "/tmp/runtime",
            "--out",
            "/tmp/link",
            "--inputs-fingerprint",
            "00",
            "--surprise",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("link-cone 不接受参数"));
    }
}
