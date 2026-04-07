//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前阶段（T0802～T0808）支持两个能力：
//! - `--emit-llvm <input.scoop> [-o <out.ll>]`：生成 LLVM IR（`main` v1 子集 codegen）。
//! - `--emit-obj <input.scoop> [-o <out.o>]`：把 module 编译为 object 文件（为链接做准备）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

const USAGE: &str = "\
用法：
  scoopc --emit-llvm <input.scoop> [-o <out.ll>]
  scoopc --emit-obj  <input.scoop> [-o <out.o>]

说明：
  - 该二进制需要启用 `scoopc` 的 `llvm` feature（需要 LLVM 21.1 + `llvm-config`）。
  - 当前只 codegen 入口 `fun main` 的一小部分表达式子集；其它顶层声明会被忽略。
";

fn main() -> Result<()> {
    let mut emit_llvm = false;
    let mut emit_obj = false;
    let mut output: Option<PathBuf> = None;
    let mut input: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                return Ok(());
            }
            "--emit-llvm" => emit_llvm = true,
            "--emit-obj" => emit_obj = true,
            "-o" | "--output" => {
                let Some(v) = args.next() else {
                    return Err(miette::miette!("参数 `{arg}` 需要一个输出路径\n\n{USAGE}"));
                };
                output = Some(PathBuf::from(v));
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

    let mode = match (emit_llvm, emit_obj) {
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
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = output.unwrap_or_else(|| match mode {
        EmitMode::LlvmIr => default_ll_path(&input),
        EmitMode::Object => default_obj_path(&input),
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("无法创建输出目录")?;
    }

    let source = scoopc::source::SourceFile::load(&input)?;
    let session = scoopc::session::Session::new()?;

    match mode {
        EmitMode::LlvmIr => {
            scoopc::llvm::emit_minimal_main_ir_to_file(&session, &source, &output)?;
            eprintln!("已写入 LLVM IR：{}", output.display());
        }
        EmitMode::Object => {
            scoopc::llvm::emit_minimal_main_obj_to_file(&session, &source, &output)?;
            eprintln!("已写入 object 文件：{}", output.display());
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmitMode {
    LlvmIr,
    Object,
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
