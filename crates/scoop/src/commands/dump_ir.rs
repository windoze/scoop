//! `scoop dump-ir` 子命令。
//!
//! 当前阶段：输出“monomorphic MIR instances”的 Debug 视图，用于验证：
//! - `InstanceKey` 是否独立于最终 backend 符号名；
//! - generic MIR template 是否在 MIR 层 materialize 成稳定实例；
//! - direct-call fixed-point / nested closure family 重写是否成立。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 读取输入文件并打印实例化后的 MIR Debug 输出。
pub fn run(input: PathBuf) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::new()?;
    let lowered = scoopc::mir::materialize_for_dump(&session, &file)
        .map_err(|err| miette::Report::from(*err))?;
    println!("{:#?}", lowered);
    Ok(())
}
