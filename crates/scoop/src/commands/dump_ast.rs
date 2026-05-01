//! `scoop dump-ast` 子命令。
//!
//! 当前阶段：输出“早期 AST”（文件头 + 顶层声明 + `Block { stmts }` 的最小语句子集）。
//! 后续会逐步扩展为完整 AST/HIR dump。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

fn load_ast_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<scoopc::ast::File> {
    scoopc::effect_refactor_pipeline::parse_ast_for_dump(session, source)
        .map_err(miette::Report::from)
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let ast = load_ast_for_dump(&session, &file)?;
    println!("{ast:#?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoopc::session::{EffectPipelineMode, Session, SessionOptions};
    use scoopc::source::SourceFile;

    #[test]
    fn dump_ast_command_uses_refactor_ast_dispatcher() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let ast = super::load_ast_for_dump(&session, &source).unwrap();
        let stage = scoopc::effect_refactor_pipeline::dispatcher_for_session(&session).ast();

        assert!(ast.package.is_some());
        assert_eq!(stage.mode(), EffectPipelineMode::Refactor);
    }
}
