//! 命令行参数定义。
//!
//! 本模块只负责“解析参数 → 结构化配置”，不做具体业务逻辑。

use std::num::NonZeroU32;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scoopc::session::{EffectPipelineMode, ParseEffectPipelineModeError};

#[derive(Debug, Parser)]
#[command(name = "scoop", version, about = "Scoop compiler + tooling")]
pub struct Args {
    /// 显式选择 effect 主线；缺省保持 legacy。
    #[arg(
        long = "effect-pipeline",
        global = true,
        value_name = "MODE",
        default_value_t = EffectPipelineMode::Legacy,
        value_parser = parse_effect_pipeline_mode,
    )]
    pub effect_pipeline: EffectPipelineMode,

    #[command(subcommand)]
    pub command: Command,
}

fn parse_effect_pipeline_mode(value: &str) -> Result<EffectPipelineMode, String> {
    value
        .parse()
        .map_err(|err: ParseEffectPipelineModeError| err.to_string())
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 创建新的 CONE 项目骨架（application）
    New {
        /// 项目名（同时用作目录名与 `[cone].name`）
        project_name: String,
    },

    /// 运行 fixtures（当前阶段仅做最小 smoke）
    Test {
        /// fixtures 目录或单个 fixture 文件（默认：`tests/fixtures`）
        #[arg(long)]
        fixtures: Option<PathBuf>,

        /// 优化等级（0|1|2|3|s|z）
        ///
        /// 说明：用于 `scoop test` 触发的 build/run-pass/build fixtures（默认随 profile 策略）。
        #[arg(short = 'O', long = "opt-level", value_name = "LEVEL")]
        opt_level: Option<String>,

        /// run-pass：在每次分配前触发额外 GC（env: `SCOOP_GC_STRESS=1`）
        #[arg(long)]
        gc_stress: bool,

        /// run-pass：强制开启 moving GC（env: `SCOOP_GC_MOVE=1`）
        #[arg(long)]
        gc_move: bool,

        /// run-pass：Immix 并行 mark/sweep worker 数（env: `SCOOP_GC_IMMIX_PARALLEL_{MARK,SWEEP}`；1=默认 4；N>=2 指定；上限 32）
        #[arg(long, value_name = "N")]
        threads: Option<NonZeroU32>,
    },

    /// 解析输入并打印 AST（当前阶段输出为占位信息）
    DumpAst {
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
    },

    /// 打包 cone 包为 `.cone` 归档（v0：只写包）
    Package {
        /// 输入包目录（包含 `Cone.toml`）
        input: PathBuf,
        /// 输出 `.cone` 文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use super::{Args, Command};
    use clap::Parser as _;
    use scoopc::session::EffectPipelineMode;

    #[test]
    fn test_effect_pipeline_defaults_to_legacy() {
        let args =
            Args::try_parse_from(["scoop", "dump-hir", "tests/fixtures/parse/minimal.scoop"])
                .unwrap();

        assert_eq!(args.effect_pipeline, EffectPipelineMode::Legacy);
    }

    #[test]
    fn test_effect_pipeline_parses_legacy() {
        let args = Args::try_parse_from([
            "scoop",
            "--effect-pipeline",
            "legacy",
            "dump-ast",
            "tests/fixtures/parse/minimal.scoop",
        ])
        .unwrap();

        assert_eq!(args.effect_pipeline, EffectPipelineMode::Legacy);
    }

    #[test]
    fn test_effect_pipeline_parses_refactor() {
        let args = Args::try_parse_from([
            "scoop",
            "--effect-pipeline",
            "refactor",
            "dump-mir",
            "tests/fixtures/parse/minimal.scoop",
        ])
        .unwrap();

        assert_eq!(args.effect_pipeline, EffectPipelineMode::Refactor);
    }

    #[test]
    fn test_effect_pipeline_rejects_invalid_value() {
        let err = Args::try_parse_from([
            "scoop",
            "--effect-pipeline",
            "future",
            "dump-ir",
            "tests/fixtures/parse/minimal.scoop",
        ])
        .unwrap_err();

        assert!(matches!(
            err.kind(),
            clap::error::ErrorKind::ValueValidation | clap::error::ErrorKind::InvalidValue
        ));
    }

    #[test]
    fn test_command_parses_build_profile_default() {
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
    fn test_command_parses_build_profile_release() {
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
    fn test_command_rejects_conflicting_build_profile_flags() {
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
    fn test_command_parses_build_opt_level() {
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
    fn test_command_parses_run_pass_gc_flags() {
        let args = Args::try_parse_from([
            "scoop",
            "test",
            "-O2",
            "--gc-stress",
            "--gc-move",
            "--threads",
            "4",
        ])
        .unwrap();

        match args.command {
            Command::Test {
                fixtures,
                opt_level,
                gc_stress,
                gc_move,
                threads,
            } => {
                assert!(fixtures.is_none());
                assert_eq!(opt_level.as_deref(), Some("2"));
                assert!(gc_stress);
                assert!(gc_move);
                assert_eq!(threads.unwrap().get(), 4);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn test_command_parses_run_profile_default_and_optional_input() {
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
    fn test_command_parses_run_profile_release() {
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
    fn test_command_rejects_conflicting_run_profile_flags() {
        let err = Args::try_parse_from(["scoop", "run", "--debug", "--release"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}
