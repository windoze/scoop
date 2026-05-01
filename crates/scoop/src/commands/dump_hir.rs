//! `scoop dump-hir` 子命令。
//!
//! 当前阶段（TODO T0701）：输出 HIR 的 Debug 视图，用于后续 HIR/MIR/LLVM 迭代调试。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{EffectPipelineMode, SessionOptions};

enum DumpHirOutput {
    Legacy(scoopc::hir::LoweredHir),
    Refactor(scoopc::effect_refactor_pipeline::TypedHirStageOutput),
}

impl DumpHirOutput {
    fn hir_file(&self) -> &scoopc::hir::File {
        match self {
            Self::Legacy(lowered) => &lowered.file,
            Self::Refactor(output) => output.hir_file(),
        }
    }
}

fn load_hir_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<DumpHirOutput> {
    match session.effect_pipeline_mode() {
        EffectPipelineMode::Legacy => scoopc::hir::lower_for_dump(session, source)
            .map(DumpHirOutput::Legacy)
            .map_err(miette::Report::from),
        EffectPipelineMode::Refactor => {
            scoopc::effect_refactor_pipeline::load_typed_hir_stage_output_for_dump(session, source)
                .map(DumpHirOutput::Refactor)
                .map_err(miette::Report::from)
        }
    }
}

/// 读取输入文件并打印 HIR（Debug）。
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
    Ok(format!("{:#?}\n", lowered.hir_file()))
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    print!("{}", render_dump_output(input, session_options)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DumpHirOutput;
    use scoopc::session::{EffectPipelineMode, Session, SessionOptions};
    use scoopc::source::SourceFile;

    #[test]
    fn refactor_typed_hir_stage_dump_hir_command_uses_new_stage() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let hir_output = super::load_hir_for_dump(&session, &source).unwrap();
        let stage = scoopc::effect_refactor_pipeline::dispatcher_for_session(&session).typed_hir();

        match hir_output {
            DumpHirOutput::Legacy(_) => panic!("refactor dump-hir 不应走 legacy lower_for_dump"),
            DumpHirOutput::Refactor(output) => {
                assert_eq!(output.hir_file().items.len(), 1);
                assert!(output.effect_contracts().is_placeholder());
            }
        }
        assert_eq!(stage.mode(), EffectPipelineMode::Refactor);
    }

    #[test]
    fn legacy_dump_hir_command_keeps_lower_for_dump_behavior() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Legacy)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let hir_output = super::load_hir_for_dump(&session, &source).unwrap();
        let legacy = scoopc::hir::lower_for_dump(&session, &source).unwrap();

        match hir_output {
            DumpHirOutput::Legacy(lowered) => {
                assert_eq!(
                    format!("{:#?}\n", lowered.file),
                    format!("{:#?}\n", legacy.file)
                );
            }
            DumpHirOutput::Refactor(_) => panic!("legacy dump-hir 不应走 refactor typed HIR stage"),
        }
    }
}
