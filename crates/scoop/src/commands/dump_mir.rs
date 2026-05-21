//! `scoop dump-mir` 子命令。
//!
//! 当前输出的是 generic early MIR / ANF template 的稳定文本视图，用于验证：
//! - backend-agnostic 的 CFG / locals / call kind / perform / pattern lowering 形状；
//! - `dump-mir` 仍停留在 generic template 边界，不提前 materialize `::<T>` 实例；
//! - 更晚 backend lowering（如 LLVM 细节）不会混入这层输出。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

fn load_mir_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<Box<scoopc::pipeline::MirStageOutput>> {
    scoopc::pipeline::load_direct_style_mir_stage_output_for_dump(session, source)
        .map(Box::new)
        .map_err(miette::Report::from)
}

/// 读取输入文件并打印 MIR 稳定文本视图。
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
    let output = load_mir_for_dump(&session, &file)?;
    Ok(output.stable_dump())
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
    fn dump_mir_command_uses_single_direct_mir_stage() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let mir_output = super::load_mir_for_dump(&session, &source).unwrap();
        assert!(mir_output.callable_body("sample.main").is_some());
        assert!(mir_output.callable_body("sample.helper").is_some());
        assert!(mir_output.stable_dump().contains("FunDecl"));
    }

    #[test]
    fn dump_mir_render_uses_stage_stable_dump_surface() {
        let session_options = SessionOptions::new();
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mir_lowered/top_level_roots.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();
        let session = Session::with_options(session_options.clone()).unwrap();

        let expected =
            scoopc::pipeline::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .stable_dump();
        let actual = super::render_dump_output(fixture, session_options).unwrap();

        assert_eq!(actual, expected);
    }
}
