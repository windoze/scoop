//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前阶段（T0802）只支持一个能力：
//! - `--emit-llvm <input.scoop> [-o <out.ll>]`：生成最小 LLVM IR（空 `main` 返回 0）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

const USAGE: &str = "\
用法：
  scoopc --emit-llvm <input.scoop> [-o <out.ll>]

说明：
  - 该二进制需要启用 `scoopc` 的 `llvm` feature（并安装对应 LLVM/llvm-config）。
  - 当前仅生成最小 module：i32 @main() { ret i32 0 }。
";

fn main() -> Result<()> {
    let mut emit_llvm = false;
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

    if !emit_llvm {
        return Err(miette::miette!("当前阶段仅支持 `--emit-llvm`\n\n{USAGE}"));
    }

    let input = input.ok_or_else(|| miette::miette!("缺少输入文件\n\n{USAGE}"))?;
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = output.unwrap_or_else(|| default_ll_path(&input));
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("无法创建输出目录")?;
    }

    let source = scoopc::source::SourceFile::load(&input)?;
    let session = scoopc::session::Session::new()?;
    scoopc::llvm::emit_minimal_main_ir_to_file(&session, &source, &output)?;

    eprintln!("已写入 LLVM IR：{}", output.display());
    Ok(())
}

fn default_ll_path(input: &Path) -> PathBuf {
    let mut out = input.to_path_buf();
    out.set_extension("ll");
    out
}

