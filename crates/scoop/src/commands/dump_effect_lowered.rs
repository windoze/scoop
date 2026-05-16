//! `scoop dump-effect-lowered` 子命令。
//!
//! 该命令是 P5 late-lowering stage 的用户可见 dump 入口：
//! - 唯一主线路径直接进入 late-lowering stage，并输出稳定的 post-opt late-lowered 文本；
//! - fixture runner 复用这里的同一 helper，避免 CLI 与 golden 各自拼接不同文本。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{Session, SessionOptions};
use scoopc::source::SourceFile;

pub(crate) fn render_effect_lowered_output(
    session: &Session,
    source: &SourceFile,
) -> Result<String> {
    let output = scoopc::pipeline::load_effect_lowered_stage_output_for_dump(session, source)
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

    use scoopc::session::{Session, SessionOptions};
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
    fn dump_effect_lowered_command_uses_late_lowering_stage_output() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = dump_fixture_source();

        let rendered = super::render_effect_lowered_output(&session, &source).unwrap();

        assert!(rendered.contains("EffectLoweredStageOutput"));
        assert!(rendered.contains("opt_level: O2"));
        assert!(rendered.contains("post_opt_program:"));
        assert!(rendered.contains("LateLoweredProgram"));
        assert!(rendered.contains("step_types:"));
        assert!(rendered.contains("continuation_objects:"));
        assert!(rendered.contains("authoritative_surface_resume_dispatch_inventory:"));
        assert!(rendered.contains("resume_packing_interfaces:"));
        assert!(rendered.contains("callables:"));
        assert!(rendered.contains("state_graph:"));
        assert!(rendered.contains("frame_schema:"));
        assert!(rendered.contains("boundary_map:"));
        assert!(rendered.contains("resume_state_map:"));
    }

    #[test]
    fn dump_effect_lowered_output_avoids_workspace_absolute_paths() {
        let fixture =
            workspace_fixture("tests/fixtures/effect_lowered/handle_finally_boundary.scoop");
        let absolute = fixture.canonicalize().unwrap();
        let rendered = super::render_dump_output(fixture, SessionOptions::new()).unwrap();

        assert!(rendered.contains("fixtures.mir_lowered.body_completes"));
        assert!(!rendered.contains(&absolute.display().to_string()));
    }
}
