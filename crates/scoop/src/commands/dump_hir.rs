//! `scoop dump-hir` 子命令。
//!
//! 输出 typed HIR 的稳定文本视图，用于 fixtures、CLI 与阶段回归检查。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

fn load_hir_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<Box<scoopc::pipeline::HirStageOutput>> {
    scoopc::pipeline::load_hir_stage_output_for_dump(session, source)
        .map(Box::new)
        .map_err(miette::Report::from)
}

/// 读取输入文件并打印 HIR 稳定文本视图。
pub(super) fn render_dump_output(
    input: PathBuf,
    session_options: SessionOptions,
) -> Result<String> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let lowered = load_hir_for_dump(&session, &file)?;
    Ok(lowered.stable_dump())
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
    fn dump_hir_command_uses_single_typed_hir_stage() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let hir_output = super::load_hir_for_dump(&session, &source).unwrap();

        assert_eq!(hir_output.hir_file().items.len(), 1);
        assert!(!hir_output.hir_facts().contract_bridge.is_empty());
        assert_eq!(hir_output.hir_facts().contract_bridge.function_effects, 1);
        assert!(hir_output.stable_dump().contains("hir_facts {"));
        assert!(hir_output.stable_dump().contains("typed_contract_bridge {"));
    }

    #[test]
    fn dump_hir_output_appends_typed_contract_section() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package sample

import scoop.core.*

fun use(k: Continuation<Int, Int, eff Pure>): Int / Raise<RuntimeError> {
    k.resume(1)
}
"#,
        );

        let rendered = super::load_hir_for_dump(&session, &source)
            .unwrap()
            .stable_dump();

        assert!(rendered.contains("hir_facts {"));
        assert!(rendered.contains("typed_contract_bridge {"));
        assert!(rendered.contains("continuation_resume_sites"));
        assert!(rendered.contains("required_effects: scoop.core.Raise<scoop.core.RuntimeError>"));
    }
}
