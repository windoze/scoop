//! `scoop dump-effect-facts` 子命令。
//!
//! 该命令是 P4 effect-facts stage 的用户可见 dump 入口：
//! - 唯一主线路径直接进入 effect-facts stage，并输出稳定的 handoff/facts 文本；
//! - fixture runner 复用这里的同一 helper，避免 CLI 与 golden 各自拼接不同文本。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::{Session, SessionOptions};
use scoopc::source::SourceFile;

pub(crate) fn render_effect_facts_output(session: &Session, source: &SourceFile) -> Result<String> {
    let output = scoopc::pipeline::load_effect_facts_stage_output_for_dump(session, source)
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
    render_effect_facts_output(&session, &file)
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
            "<mem>/dump_effect_facts_fixture.scoop",
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
    fn dump_effect_facts_command_uses_effect_facts_stage_output() {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = dump_fixture_source();

        let rendered = super::render_effect_facts_output(&session, &source).unwrap();

        assert!(rendered.contains("MaterializedEffectFacts"));
        assert!(rendered.contains("step_schemas:"));
        assert!(rendered.contains("callable_facts:"));
        assert!(rendered.contains("body_facts:"));
        assert!(rendered.contains("kind: Resume"));
    }

    #[test]
    fn dump_effect_facts_output_normalizes_workspace_absolute_paths() {
        let fixture = workspace_fixture("tests/fixtures/effect_facts/handle_perform.scoop");
        let rendered = super::render_dump_output(fixture, SessionOptions::new()).unwrap();

        assert!(
            rendered
                .contains("ContinuationObject@tests/fixtures/effect_facts/handle_perform.scoop")
        );
        assert!(!rendered.contains("/tests/fixtures/effect_facts/handle_perform.scoop"));
    }
}
