//! `scoop dump-ir` 子命令。
//!
//! 当前阶段：输出 materialized MIR instances 的 Debug 视图，用于验证：
//! - `InstanceKey` 是否独立于最终 backend 符号名；
//! - generic MIR template 是否在 MIR 层 materialize 成稳定实例；
//! - 单一路径不再拼接已删除的 raw/codegen route 兼容输出。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{Session, SessionOptions};
use scoopc::source::SourceFile;

fn render_materialized_ir_output(session: &Session, source: &SourceFile) -> Result<String> {
    let materialized = scoopc::pipeline::materialize_direct_style_mir_for_dump(session, source)
        .map_err(|err| miette::Report::from(*err))?;
    Ok(format!("{materialized:#?}"))
}

/// 读取输入文件并打印实例化后的 MIR Debug 输出。
pub(super) fn render_dump_output(
    input: PathBuf,
    session_options: SessionOptions,
) -> Result<String> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = SourceFile::load(&input)?;

    let session = Session::with_options(session_options)?;
    render_materialized_ir_output(&session, &file)
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    print!("{}", render_dump_output(input, session_options)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoopc::session::{Session, SessionOptions};
    use scoopc::source::SourceFile;

    #[test]
    fn dump_ir_command_uses_materialized_mir_output_only() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/dump_ir_fixture.scoop",
            r#"
package sample

fun <T> id(value: T): T {
    return value
}

fun main(): Int {
    return id<Int>(1)
}
"#,
        );

        let actual = super::render_materialized_ir_output(&session, &source).unwrap();

        assert!(actual.contains("MaterializedMir"));
        assert!(actual.contains("sample.id::<Int>"));
        assert!(actual.contains("sample.main"));
        assert!(!actual.contains("MirCodegenRoutingFacts"));
    }
}
