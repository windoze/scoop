//! `scoop dump-ir` 子命令。
//!
//! 当前阶段（TODO T0712）：输出“单态化实例”的 MIR Debug 视图，用于验证：
//! - 泛型函数调用点是否能收集到正确的 `MonomorphKey`
//! - 同一个泛型函数在不同 type args 下是否会生成不同实例

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 读取输入文件并打印 IR（当前阶段：monomorphized MIR 的 Debug 输出）。
pub fn run(input: PathBuf) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::new()?;
    let lowered = scoopc::monomorph::lower_for_dump(&session, &file)
        .map_err(|err| miette::Report::from(*err))?;
    println!("{:#?}", lowered.file);
    Ok(())
}
