//! `scoopc` CLI 参数解析。
//!
//! 该模块只负责把原始参数解析成稳定结构，避免把 session 选择逻辑散落在二进制入口里。

use std::path::PathBuf;

use miette::Result;

use crate::session::{EffectPipelineMode, ParseEffectPipelineModeError, SessionOptions};

pub const USAGE: &str = "\
用法：
  scoopc [--effect-pipeline <legacy|refactor>] --emit-llvm <input.scoop> [-o <out.ll>]
  scoopc [--effect-pipeline <legacy|refactor>] --emit-obj  <input.scoop> [-o <out.o>]

说明：
  - `--effect-pipeline` 缺省为 `refactor`；`legacy` 仅保留为短期 compare/rollback 入口。
  - 该二进制需要启用 `scoopc` 的 `llvm` feature（需要 LLVM 21.1 + `llvm-config`）。
  - 当前只 codegen 入口 `fun main` 的一小部分表达式子集；其它顶层声明会被忽略。
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerCli {
    pub emit_mode: EmitMode,
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub session_options: SessionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitMode {
    LlvmIr,
    Object,
}

pub fn parse_args<I, S>(args: I) -> Result<Option<CompilerCli>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut emit_llvm = false;
    let mut emit_obj = false;
    let mut output: Option<PathBuf> = None;
    let mut input: Option<PathBuf> = None;
    let mut effect_pipeline = EffectPipelineMode::Refactor;

    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--emit-llvm" => emit_llvm = true,
            "--emit-obj" => emit_obj = true,
            "--effect-pipeline" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "参数 `--effect-pipeline` 需要一个值\n\n{USAGE}"
                    ));
                };
                effect_pipeline = value.parse().map_err(|err: ParseEffectPipelineModeError| {
                    miette::miette!("{err}\n\n{USAGE}")
                })?;
            }
            "-o" | "--output" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("参数 `{arg}` 需要一个输出路径\n\n{USAGE}"));
                };
                output = Some(PathBuf::from(value));
            }
            _ if arg.starts_with('-') => {
                return Err(miette::miette!("未知参数：{arg}\n\n{USAGE}"));
            }
            _ => {
                if input.is_some() {
                    return Err(miette::miette!("一次只支持一个输入文件\n\n{USAGE}"));
                }
                input = Some(PathBuf::from(arg));
            }
        }
    }

    let emit_mode = match (emit_llvm, emit_obj) {
        (true, false) => EmitMode::LlvmIr,
        (false, true) => EmitMode::Object,
        (false, false) => {
            return Err(miette::miette!(
                "缺少输出模式（需要 `--emit-llvm` 或 `--emit-obj`）\n\n{USAGE}"
            ));
        }
        (true, true) => {
            return Err(miette::miette!(
                "`--emit-llvm` 与 `--emit-obj` 不能同时使用\n\n{USAGE}"
            ));
        }
    };

    let input = input.ok_or_else(|| miette::miette!("缺少输入文件\n\n{USAGE}"))?;

    Ok(Some(CompilerCli {
        emit_mode,
        input,
        output,
        session_options: SessionOptions::new(effect_pipeline),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{EmitMode, parse_args};
    use crate::session::EffectPipelineMode;

    #[test]
    fn default_effect_pipeline_is_refactor_for_scoopc_cli() {
        let cli = parse_args(["--emit-llvm", "input.scoop"]).unwrap().unwrap();

        assert_eq!(cli.emit_mode, EmitMode::LlvmIr);
        assert_eq!(cli.input, PathBuf::from("input.scoop"));
        assert_eq!(
            cli.session_options.effect_pipeline,
            EffectPipelineMode::Refactor
        );
    }

    #[test]
    fn explicit_legacy_pipeline_still_available_for_scoopc_cli() {
        let cli = parse_args(["--effect-pipeline", "legacy", "--emit-obj", "input.scoop"])
            .unwrap()
            .unwrap();

        assert_eq!(cli.emit_mode, EmitMode::Object);
        assert_eq!(
            cli.session_options.effect_pipeline,
            EffectPipelineMode::Legacy
        );
    }

    #[test]
    fn explicit_refactor_pipeline_still_available_for_scoopc_cli() {
        let cli = parse_args([
            "--effect-pipeline",
            "refactor",
            "--emit-llvm",
            "input.scoop",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(cli.emit_mode, EmitMode::LlvmIr);
        assert_eq!(
            cli.session_options.effect_pipeline,
            EffectPipelineMode::Refactor
        );
    }

    #[test]
    fn parse_args_rejects_invalid_effect_pipeline() {
        let err =
            parse_args(["--effect-pipeline", "future", "--emit-llvm", "input.scoop"]).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("legacy"));
        assert!(message.contains("refactor"));
    }
}
