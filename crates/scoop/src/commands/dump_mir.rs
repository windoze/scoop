//! `scoop dump-mir` 子命令。
//!
//! 当前输出的是 generic early MIR / ANF template 的 Debug 视图，用于验证：
//! - backend-agnostic 的 CFG / locals / call kind / perform / pattern lowering 形状；
//! - `dump-mir` 仍停留在 generic template 边界，不提前 materialize `::<T>` 实例；
//! - 更晚 backend lowering（如 LLVM 细节）不会混入这层输出。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{EffectPipelineMode, SessionOptions};

enum DumpMirOutput {
    Legacy(Box<scoopc::mir::LoweredMir>),
    Refactor(Box<scoopc::effect_refactor_pipeline::RefactorMirStageOutput>),
}

impl DumpMirOutput {
    fn render(&self) -> String {
        match self {
            Self::Legacy(lowered) => format!("{:#?}\n", lowered.file),
            Self::Refactor(output) => output.stable_dump(),
        }
    }
}

fn load_mir_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<DumpMirOutput> {
    match session.effect_pipeline_mode() {
        EffectPipelineMode::Legacy => scoopc::mir::lower_for_dump(session, source)
            .map(|lowered| DumpMirOutput::Legacy(Box::new(lowered)))
            .map_err(miette::Report::from),
        EffectPipelineMode::Refactor => {
            scoopc::effect_refactor_pipeline::load_direct_style_mir_stage_output_for_dump(
                session, source,
            )
            .map(|output| DumpMirOutput::Refactor(Box::new(output)))
            .map_err(miette::Report::from)
        }
    }
}

/// 读取输入文件并打印 MIR（Debug）。
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
    Ok(output.render())
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    print!("{}", render_dump_output(input, session_options)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DumpMirOutput;
    use scoopc::session::{EffectPipelineMode, Session, SessionOptions};
    use scoopc::source::SourceFile;

    #[test]
    fn refactor_direct_mir_stage_dump_mir_command_uses_new_stage() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let mir_output = super::load_mir_for_dump(&session, &source).unwrap();
        let stage =
            scoopc::effect_refactor_pipeline::dispatcher_for_session(&session).direct_style_mir();

        match mir_output {
            DumpMirOutput::Legacy(_) => panic!("refactor dump-mir 不应走 legacy lower_for_dump"),
            DumpMirOutput::Refactor(output) => {
                assert!(output.callable_body("sample.main").is_some());
                assert_eq!(output.effect_contracts().function_effects().len(), 2);
                assert!(output.stable_dump().contains("FunDecl"));
            }
        }
        assert_eq!(stage.mode(), EffectPipelineMode::Refactor);
    }

    #[test]
    fn refactor_dump_mir_render_uses_stage_stable_dump_surface() {
        let session_options = SessionOptions::new(EffectPipelineMode::Refactor);
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mir_refactor/top_level_roots.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();
        let session = Session::with_options(session_options).unwrap();

        let expected =
            scoopc::effect_refactor_pipeline::load_direct_style_mir_stage_output_for_dump(
                &session, &source,
            )
            .unwrap()
            .stable_dump();
        let actual = super::render_dump_output(fixture, session_options).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn legacy_dump_mir_command_keeps_lower_for_dump_behavior() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Legacy)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let mir_output = super::load_mir_for_dump(&session, &source).unwrap();
        let legacy = scoopc::mir::lower_for_dump(&session, &source).unwrap();

        match mir_output {
            DumpMirOutput::Legacy(lowered) => {
                assert_eq!(
                    format!("{:#?}\n", lowered.file),
                    format!("{:#?}\n", legacy.file)
                );
            }
            DumpMirOutput::Refactor(_) => panic!("legacy dump-mir 不应走 refactor MIR stage"),
        }
    }
}
