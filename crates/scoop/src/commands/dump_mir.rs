//! `scoop dump-mir` 子命令。
//!
//! 当前输出的是 generic early MIR / ANF template 的 Debug 视图，用于验证：
//! - backend-agnostic 的 CFG / locals / call kind / perform / pattern lowering 形状；
//! - `dump-mir` 仍停留在 generic template 边界，不提前 materialize `::<T>` 实例；
//! - 更晚 backend lowering（如 LLVM 细节）不会混入这层输出。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

/// 读取输入文件并打印 MIR（Debug）。
pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let lowered =
        scoopc::effect_refactor_pipeline::lower_direct_style_mir_for_dump(&session, &file)
            .map_err(miette::Report::from)?;
    println!("{:#?}", lowered.file);
    Ok(())
}
