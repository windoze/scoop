//! `scoopc` 独立命令行入口（早期阶段）。
//!
//! 当前阶段（T0802～T0808）支持两个能力：
//! - `--emit-llvm <input.scoop> [-o <out.ll>]`：生成 LLVM IR（`main` v1 子集 codegen）。
//! - `--emit-obj <input.scoop> [-o <out.o>]`：把 module 编译为 object 文件（为链接做准备）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

fn main() -> Result<()> {
    let Some(cli) = scoopc::driver_cli::parse_args(std::env::args().skip(1))? else {
        eprintln!("{}", scoopc::driver_cli::USAGE);
        return Ok(());
    };

    let input = cli
        .input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let output = cli.output.unwrap_or_else(|| match cli.emit_mode {
        scoopc::driver_cli::EmitMode::LlvmIr => default_ll_path(&input),
        scoopc::driver_cli::EmitMode::Object => default_obj_path(&input),
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("无法创建输出目录")?;
    }

    let source = scoopc::source::SourceFile::load(&input)?;
    let session = scoopc::session::Session::with_options(cli.session_options)?;

    match cli.emit_mode {
        scoopc::driver_cli::EmitMode::LlvmIr => {
            scoopc::llvm::emit_minimal_main_ir_to_file(&session, &source, &output)?;
            eprintln!("已写入 LLVM IR：{}", output.display());
        }
        scoopc::driver_cli::EmitMode::Object => {
            scoopc::llvm::emit_minimal_main_obj_to_file(&session, &source, &output)?;
            eprintln!("已写入 object 文件：{}", output.display());
        }
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
