//! 命令行参数定义。
//!
//! 本模块只负责“解析参数 → 结构化配置”，不做具体业务逻辑。

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "scoop", version, about = "Scoop compiler + tooling")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 创建新的 CONE 项目骨架（application）
    New {
        /// 项目名（同时用作目录名与 `[cone].name`）
        project_name: String,
    },

    /// 解析输入并打印 AST（当前阶段输出为占位信息）
    DumpAst {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve/typecheck/materialize 输入并打印 effect facts
    DumpEffectFacts {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve/typecheck/materialize 输入并打印 late-lowered representation
    DumpEffectLowered {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve 输入并打印 HIR（早期实现：Debug 输出）
    DumpHir {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve 输入并打印 MIR（早期实现：Debug 输出）
    DumpMir {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve/typecheck 输入并打印 IR（单态化实例的 MIR 视图）
    DumpIr {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve 输入并打印 RTTI/type descriptor（v0：type_id + parent chain + trace bitmap/trace_fn）
    DumpRtti {
        /// 输入源文件路径
        input: PathBuf,
        /// 指定要打印的类型名（FQN 或 simple name；省略则打印文件内所有可生成 RTTI 的类型）
        #[arg(long = "type", value_name = "TYPE")]
        type_name: Option<String>,
    },

    /// 从二进制产物中读取并打印 LLVM stackmap header 信息
    DumpStackmaps {
        /// 输入可执行文件路径（Mach-O/ELF）
        input: PathBuf,

        /// 校验 stackmap roots 语义契约（GC-FIX Phase A1）
        ///
        /// 失败时会返回结构化诊断（指出哪个 record/location 不符合契约）。
        #[arg(long)]
        verify_roots: bool,

        /// 打印每条 stackmap record 的 locations 明细（用于排查 roots 误判等问题）
        ///
        /// 说明：默认只输出稳定 header；启用该开关后会输出额外调试信息（不建议用于 fixtures）。
        #[arg(long)]
        dump_records: bool,
    },

    /// 构建可执行文件（默认启用 LLVM 后端；如用 `--no-default-features` 则仅做前端检查）
    Build {
        /// 输入源文件路径（.scoop）或包目录（包含 Cone.toml）
        input: PathBuf,
        /// 输出文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// （cone 包模式）指定入口 package（覆盖 `Cone.toml` 的 `native-build.entry-package`）
        #[arg(long = "entry-package", value_name = "PACKAGE")]
        entry_package: Option<String>,

        /// 选择 debug profile（默认；便于脚本化）
        #[arg(long, conflicts_with = "release")]
        debug: bool,

        /// 选择 release profile
        #[arg(long, conflicts_with = "debug")]
        release: bool,

        /// 优化等级（0|1|2|3|s|z）
        ///
        /// - 若输入为 cone 包目录：CLI 会覆盖 `Cone.toml[native-build].opt-level`
        /// - 若未显式指定：默认随 profile（debug=0，release=2）
        #[arg(short = 'O', long = "opt-level", value_name = "LEVEL")]
        opt_level: Option<String>,

        /// 禁用粗粒度增量构建（T1124）。
        ///
        /// 也可用环境变量 `SCOOP_INCREMENTAL=0` 全局禁用。
        #[arg(long)]
        no_incremental: bool,

        /// 输出 LLVM IR（`.ll`）
        #[arg(long, conflicts_with_all = ["emit_obj", "emit_asm"])]
        emit_llvm: bool,

        /// 输出 object 文件（`.o` / `.obj`）
        #[arg(long, conflicts_with_all = ["emit_llvm", "emit_asm"])]
        emit_obj: bool,

        /// 输出汇编（`.s` / `.asm`）
        #[arg(long, conflicts_with_all = ["emit_llvm", "emit_obj"])]
        emit_asm: bool,

        /// per-cone 多进程并发编译的最大子进程数（P10-T05；本任务仅落地 CLI/trait surface，不引入并发执行行为）。
        ///
        /// 也可用环境变量 `SCOOP_BUILD_JOBS` 设置；CLI 优先级高于 env。未指定时使用 `DEFAULT_BUILD_JOBS`。
        /// 必须为正整数（0 与负值会被拒绝）。
        #[arg(short = 'j', long = "jobs", value_name = "N")]
        jobs: Option<NonZeroUsize>,

        /// 额外启用的 sysroot source cone（可重复；CLI 值覆盖 SCOOP_SYSROOT_DEPS）。
        #[arg(long = "sysroot-dep", value_name = "NAME", action = ArgAction::Append)]
        sysroot_dep: Vec<String>,
    },

    /// 运行程序（先 build 后 exec；需要启用 LLVM 后端；默认已启用）
    Run {
        /// 输入源文件路径（.scoop）或包目录（包含 Cone.toml）
        ///
        /// 省略时默认使用当前目录：若当前目录（或其祖先目录）包含 `Cone.toml`，则按 cone 项目运行。
        input: Option<PathBuf>,
        /// （cone 包模式）指定入口 package（覆盖 `Cone.toml` 的 `native-build.entry-package`）
        #[arg(long = "entry-package", value_name = "PACKAGE")]
        entry_package: Option<String>,
        /// 选择 debug profile（默认；便于脚本化）
        #[arg(long, conflicts_with = "release")]
        debug: bool,
        /// 选择 release profile
        #[arg(long, conflicts_with = "debug")]
        release: bool,

        /// 优化等级（0|1|2|3|s|z）
        ///
        /// - 若输入为 cone 包目录：CLI 会覆盖 `Cone.toml[native-build].opt-level`
        /// - 若未显式指定：默认随 profile（debug=0，release=2）
        #[arg(short = 'O', long = "opt-level", value_name = "LEVEL")]
        opt_level: Option<String>,

        /// 禁用粗粒度增量构建（T1124）。
        ///
        /// 也可用环境变量 `SCOOP_INCREMENTAL=0` 全局禁用。
        #[arg(long)]
        no_incremental: bool,

        /// 传递给被运行程序的参数（建议用 `--` 与 `scoop run` 自身参数分隔）
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,

        /// per-cone 多进程并发编译的最大子进程数（P10-T05；本任务仅落地 CLI/trait surface，不引入并发执行行为）。
        ///
        /// 也可用环境变量 `SCOOP_BUILD_JOBS` 设置；CLI 优先级高于 env。未指定时使用 `DEFAULT_BUILD_JOBS`。
        /// 必须为正整数（0 与负值会被拒绝）。
        #[arg(short = 'j', long = "jobs", value_name = "N")]
        jobs: Option<NonZeroUsize>,

        /// 额外启用的 sysroot source cone（可重复；CLI 值覆盖 SCOOP_SYSROOT_DEPS）。
        #[arg(long = "sysroot-dep", value_name = "NAME", action = ArgAction::Append)]
        sysroot_dep: Vec<String>,
    },

    /// `.cone` archive packaging is temporarily unsupported during source-only cone redesign
    Package {
        /// 输入包目录（包含 `Cone.toml`）
        input: PathBuf,
        /// 输出 `.cone` 文件路径（当前不会写出）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Args, Command};
    use clap::{CommandFactory as _, Parser as _};

    #[test]
    fn effect_pipeline_selector_removed_for_scoop_cli() {
        let err = Args::try_parse_from([
            "scoop",
            "--effect-pipeline",
            "legacy",
            "dump-ast",
            "tests/fixtures/parse/minimal.scoop",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("--effect-pipeline"));
    }

    #[test]
    fn default_pipeline_parses_dump_effect_facts() {
        let args = Args::try_parse_from([
            "scoop",
            "dump-effect-facts",
            "tests/fixtures/mir_lowered/handle_perform.scoop",
        ])
        .unwrap();

        match args.command {
            Command::DumpEffectFacts { input } => {
                assert_eq!(
                    input,
                    PathBuf::from("tests/fixtures/mir_lowered/handle_perform.scoop")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn default_pipeline_parses_dump_effect_lowered() {
        let args = Args::try_parse_from([
            "scoop",
            "dump-effect-lowered",
            "tests/fixtures/effect_lowered/handle_perform.scoop",
        ])
        .unwrap();

        match args.command {
            Command::DumpEffectLowered { input } => {
                assert_eq!(
                    input,
                    PathBuf::from("tests/fixtures/effect_lowered/handle_perform.scoop")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn effect_pipeline_selector_removed_for_invalid_value_too() {
        let err = Args::try_parse_from([
            "scoop",
            "--effect-pipeline",
            "future",
            "dump-ir",
            "tests/fixtures/parse/minimal.scoop",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_subcommand_is_removed() {
        let err = Args::try_parse_from(["scoop", "test"]).unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn build_command_parses_profile_default() {
        let args =
            Args::try_parse_from(["scoop", "build", "tests/fixtures/parse/minimal.scoop"]).unwrap();

        match args.command {
            Command::Build { debug, release, .. } => {
                assert!(!debug, "默认不需要显式 `--debug`");
                assert!(!release, "默认应为 debug profile（release flag 为 false）");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn build_command_parses_profile_release() {
        let args = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--release",
        ])
        .unwrap();

        match args.command {
            Command::Build { debug, release, .. } => {
                assert!(!debug);
                assert!(release);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn build_command_rejects_conflicting_profile_flags() {
        let err = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--debug",
            "--release",
        ])
        .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn build_command_parses_opt_level() {
        let args = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "-O2",
        ])
        .unwrap();

        match args.command {
            Command::Build { opt_level, .. } => {
                assert_eq!(opt_level.as_deref(), Some("2"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_parses_profile_default_and_optional_input() {
        let args = Args::try_parse_from(["scoop", "run"]).unwrap();

        match args.command {
            Command::Run {
                input,
                debug,
                release,
                opt_level,
                ..
            } => {
                assert!(input.is_none(), "未提供 input 时应为 None");
                assert!(!debug, "默认不需要显式 `--debug`");
                assert!(!release, "默认应为 debug profile（release flag 为 false）");
                assert!(opt_level.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_parses_profile_release() {
        let args = Args::try_parse_from(["scoop", "run", "--release", "--opt-level", "2"]).unwrap();

        match args.command {
            Command::Run {
                debug,
                release,
                opt_level,
                ..
            } => {
                assert!(!debug);
                assert!(release);
                assert_eq!(opt_level.as_deref(), Some("2"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_rejects_conflicting_profile_flags() {
        let err = Args::try_parse_from(["scoop", "run", "--debug", "--release"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn build_command_accepts_jobs_long_flag() {
        let args = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--jobs",
            "3",
        ])
        .unwrap();

        match args.command {
            Command::Build { jobs, .. } => {
                assert_eq!(jobs.unwrap().get(), 3);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn build_command_accepts_jobs_short_flag() {
        let args = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "-j",
            "8",
        ])
        .unwrap();

        match args.command {
            Command::Build { jobs, .. } => {
                assert_eq!(jobs.unwrap().get(), 8);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn build_command_jobs_default_is_none() {
        let args =
            Args::try_parse_from(["scoop", "build", "tests/fixtures/parse/minimal.scoop"]).unwrap();

        match args.command {
            Command::Build { jobs, .. } => {
                assert!(
                    jobs.is_none(),
                    "未指定 --jobs 时应为 None（由 driver 解析默认值）"
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn build_command_rejects_jobs_zero() {
        let err = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--jobs",
            "0",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn build_command_rejects_jobs_non_numeric() {
        let err = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--jobs",
            "abc",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn build_command_rejects_jobs_negative() {
        // 负值 `-1` 在 clap 中被识别为未知短选项（首字符是 `-`）。这里断言用户拿到稳定的
        // UnknownArgument diagnostic，而不是无穷阻塞或 panic。
        let err = Args::try_parse_from([
            "scoop",
            "build",
            "tests/fixtures/parse/minimal.scoop",
            "--jobs",
            "-1",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn run_command_accepts_jobs_long_flag() {
        let args = Args::try_parse_from(["scoop", "run", "--jobs", "2"]).unwrap();

        match args.command {
            Command::Run { jobs, .. } => {
                assert_eq!(jobs.unwrap().get(), 2);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_accepts_jobs_short_flag() {
        let args = Args::try_parse_from(["scoop", "run", "-j", "6"]).unwrap();

        match args.command {
            Command::Run { jobs, .. } => {
                assert_eq!(jobs.unwrap().get(), 6);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_jobs_default_is_none() {
        let args = Args::try_parse_from(["scoop", "run"]).unwrap();

        match args.command {
            Command::Run { jobs, .. } => {
                assert!(
                    jobs.is_none(),
                    "未指定 --jobs 时应为 None（由 driver 解析默认值）"
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn run_command_rejects_jobs_zero() {
        let err = Args::try_parse_from(["scoop", "run", "--jobs", "0"]).unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn package_help_mentions_archive_is_unsupported() {
        let mut cmd = Args::command();
        let help = cmd
            .find_subcommand_mut("package")
            .unwrap()
            .render_long_help()
            .to_string();

        assert!(help.contains("temporarily unsupported"));
    }
}
