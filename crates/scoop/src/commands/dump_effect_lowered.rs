//! `scoop dump-effect-lowered` 子命令。
//!
//! 该命令是 P5 late-lowering stage 的用户可见 dump 入口：
//! - refactor 路径必须显式进入 late-lowering stage，并输出稳定的 post-opt late-lowered 文本；
//! - legacy 路径当前没有等价实现，因此返回稳定、可测试的不支持诊断；
//! - fixture runner 复用这里的同一 helper，避免 CLI 与 golden 各自拼接不同文本。

use std::path::PathBuf;

use miette::Diagnostic;
use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{EffectPipelineMode, Session, SessionOptions};
use scoopc::source::SourceFile;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error(
    "legacy effect pipeline 暂不支持 `dump-effect-lowered`；请使用 `--effect-pipeline refactor`"
)]
#[diagnostic(code(scoop::commands::dump_effect_lowered_legacy_unsupported))]
struct DumpEffectLoweredLegacyUnsupported;

pub(crate) fn render_effect_lowered_output(
    session: &Session,
    source: &SourceFile,
) -> Result<String> {
    if session.effect_pipeline_mode() != EffectPipelineMode::Refactor {
        return Err(DumpEffectLoweredLegacyUnsupported.into());
    }

    let output = scoopc::effect_refactor_pipeline::load_effect_lowered_stage_output_for_dump(
        session, source,
    )
    .map_err(|err| miette::miette!(err.to_string()))?;
    Ok(output.stable_dump())
}

pub(crate) fn render_dump_output(
    input: PathBuf,
    session_options: SessionOptions,
) -> Result<String> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    render_effect_lowered_output(&session, &file)
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    print!("{}", render_dump_output(input, session_options)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use scoopc::session::{EffectPipelineMode, Session, SessionOptions};
    use scoopc::source::SourceFile;

    fn dump_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/dump_effect_lowered_fixture.scoop",
            r#"
package sample

import scoop.core.*

effect Boom {
    fun next(): Int
}

fun resumeBoom(k: Continuation<Int, Unit, eff Boom>): Unit / (Raise<RuntimeError> + Boom) {
    k.resume(1)
}

fun handled(): Int {
    return handle {
        Boom.next()
    } with {
        Boom.next() -> 1
    }
}
"#,
        )
    }

    fn workspace_fixture(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    #[test]
    fn refactor_dump_effect_lowered_command_uses_late_lowering_stage_output() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = dump_fixture_source();

        let rendered = super::render_effect_lowered_output(&session, &source).unwrap();

        assert!(rendered.contains("RefactorEffectLoweredStageOutput"));
        assert!(rendered.contains("opt_level: O2"));
        assert!(rendered.contains("post_opt_program:"));
        assert!(rendered.contains("LateLoweredProgram"));
        assert!(rendered.contains("step_types:"));
        assert!(rendered.contains("resume_interfaces:"));
        assert!(rendered.contains("continuation_objects:"));
        assert!(rendered.contains("callables:"));
        assert!(rendered.contains("state_graph:"));
        assert!(rendered.contains("frame_schema:"));
        assert!(rendered.contains("boundary_map:"));
        assert!(rendered.contains("resume_state_map:"));
    }

    #[test]
    fn legacy_dump_effect_lowered_command_returns_stable_unsupported_diagnostic() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Legacy)).unwrap();
        let source = dump_fixture_source();

        let err = super::render_effect_lowered_output(&session, &source).unwrap_err();
        let rendered = err.to_string();

        assert!(rendered.contains("legacy effect pipeline 暂不支持 `dump-effect-lowered`"));
        assert!(rendered.contains("--effect-pipeline refactor"));
    }

    #[test]
    fn refactor_dump_effect_lowered_output_avoids_workspace_absolute_paths() {
        let fixture =
            workspace_fixture("tests/fixtures/effect_lowered/handle_finally_boundary.scoop");
        let absolute = fixture.canonicalize().unwrap();
        let rendered =
            super::render_dump_output(fixture, SessionOptions::new(EffectPipelineMode::Refactor))
                .unwrap();

        assert!(rendered.contains("fixtures.mir_refactor.body_completes"));
        assert!(!rendered.contains(&absolute.display().to_string()));
    }
}
