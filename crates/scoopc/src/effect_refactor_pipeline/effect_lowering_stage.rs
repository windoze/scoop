use std::fmt::Write;

use crate::effect_facts::{MaterializedEffectFacts, MirSnapshotBinding};
use crate::effect_lowered::{EffectLoweringError, LateLoweredProgram, LateLoweredProgramBuilder};
use crate::mir::{MaterializedMir, MaterializedMirPassView};
use crate::ty::TypeStore;

use super::RefactorEffectFactsStageOutput;

/// refactor late-lowering stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P5/P6 及后续阶段直接消费：
/// - 输入必须是 P4 的 `RefactorEffectFactsStageOutput`；
/// - stage 只消费 canonical MIR snapshot + `MaterializedEffectFacts`，不回 HIR/typecheck；
/// - `program()` 返回独立的 `LateLoweredProgram`，后续结构性 rewrite 必须继续在这份
///   late-lowered IR 上工作，而不是回头 patch P3/P4 产物；
/// - 对外暴露的输出不会混有“部分 callable 已 lowered、部分仍停留在 direct-style”的半成品状态；
/// - P6 只应把这份输出翻译到 LLVM，而不是再重做高层 effect lowering 设计。
#[derive(Debug)]
pub struct RefactorEffectLoweredStageOutput {
    effect_facts_stage_output: RefactorEffectFactsStageOutput,
    program: LateLoweredProgram,
}

impl RefactorEffectLoweredStageOutput {
    fn new(
        effect_facts_stage_output: RefactorEffectFactsStageOutput,
        program: LateLoweredProgram,
    ) -> Self {
        Self {
            effect_facts_stage_output,
            program,
        }
    }

    pub fn effect_facts_stage_output(&self) -> &RefactorEffectFactsStageOutput {
        &self.effect_facts_stage_output
    }

    pub fn snapshot_binding(&self) -> &MirSnapshotBinding {
        self.effect_facts().snapshot_binding()
    }

    pub fn materialized_mir(&self) -> &MaterializedMir {
        self.effect_facts_stage_output.materialized_mir()
    }

    pub fn materialized_pass_view(&self) -> MaterializedMirPassView<'_> {
        self.effect_facts_stage_output.materialized_pass_view()
    }

    pub fn types(&self) -> &TypeStore {
        self.effect_facts_stage_output.types()
    }

    pub fn effect_facts(&self) -> &MaterializedEffectFacts {
        self.effect_facts_stage_output.effect_facts()
    }

    pub fn program(&self) -> &LateLoweredProgram {
        &self.program
    }

    /// `dump-effect-lowered` / snapshot / P6 复用的稳定文本 surface。
    pub fn stable_dump(&self) -> String {
        render_stage_output(self)
    }

    pub fn into_parts(self) -> (RefactorEffectFactsStageOutput, LateLoweredProgram) {
        (self.effect_facts_stage_output, self.program)
    }
}

pub(crate) fn run(
    effect_facts_stage_output: RefactorEffectFactsStageOutput,
) -> Result<RefactorEffectLoweredStageOutput, EffectLoweringError> {
    let program = LateLoweredProgramBuilder::from_canonical_inputs(
        effect_facts_stage_output.materialized_pass_view(),
        effect_facts_stage_output.effect_facts(),
        effect_facts_stage_output.types(),
    )
    .build()?;
    Ok(RefactorEffectLoweredStageOutput::new(
        effect_facts_stage_output,
        program,
    ))
}

fn render_stage_output(output: &RefactorEffectLoweredStageOutput) -> String {
    let binding = output.snapshot_binding();
    let mut rendered = String::new();
    writeln!(&mut rendered, "RefactorEffectLoweredStageOutput").unwrap();
    writeln!(&mut rendered, "snapshot_binding:").unwrap();
    writeln!(
        &mut rendered,
        "  query_surface: {:?}",
        binding.query_surface()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  instance_count: {}",
        binding.instance_count()
    )
    .unwrap();
    writeln!(&mut rendered, "  canonical_body_fqns:").unwrap();
    if binding.canonical_body_fqns().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for fqn in binding.canonical_body_fqns() {
            writeln!(&mut rendered, "    - {fqn}").unwrap();
        }
    }
    rendered.push_str(&output.program().stable_dump());
    rendered
}

#[cfg(test)]
mod tests {
    use super::RefactorEffectLoweredStageOutput;
    use crate::effect_facts::CanonicalMirQuerySurface;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_lowered_stage_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    fn run_sample() -> RefactorEffectLoweredStageOutput {
        let session = refactor_session();
        let source = sample_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        super::run(effect_facts_output).expect("fixture 应可通过 refactor late-lowering stage")
    }

    #[test]
    fn refactor_effect_lowered_stage_output_is_constructible() {
        let output = run_sample();

        assert_eq!(
            output.snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(
            output.program().len(),
            output.effect_facts().callable_facts().len()
        );
        assert!(output.program().callable("sample.helper").is_some());
        assert!(output.program().callable("sample.main").is_some());
        assert!(
            output
                .stable_dump()
                .contains("RefactorEffectLoweredStageOutput")
        );
        assert!(output.stable_dump().contains("LateLoweredProgram"));
    }

    #[test]
    fn refactor_effect_lowered_stage_explicitly_consumes_p4_effect_facts_stage_output() {
        let session = refactor_session();
        let source = sample_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let main_key = effect_facts_output
            .materialized_pass_view()
            .owner_of_callable("sample.main")
            .expect("sample.main 应有 canonical owner")
            .clone();
        let main_facts = effect_facts_output
            .effect_facts()
            .callable_facts()
            .get(&main_key)
            .expect("P4 output 应发布 sample.main callable facts");
        let expected_step_schema = main_facts.step_schema();
        let expected_impl_plan = main_facts.impl_plan();
        let expected_needs_reentry = main_facts.needs_reentry();
        let expected_cases = main_facts.resolved_outward_cases().tags().to_vec();

        let output = super::run(effect_facts_output).unwrap();
        let lowered_main = output
            .program()
            .callable("sample.main")
            .expect("late-lowered program 应保留 sample.main 边界记录");

        assert_eq!(lowered_main.step_schema(), expected_step_schema);
        assert_eq!(lowered_main.impl_plan(), expected_impl_plan);
        assert_eq!(lowered_main.needs_reentry(), expected_needs_reentry);
        assert_eq!(
            lowered_main.resolved_outward_cases(),
            expected_cases.as_slice()
        );
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.program().len()
        );
    }

    #[test]
    fn refactor_effect_lowered_stage_has_no_legacy_state_machine_or_llvm_imports() {
        let stage_source = include_str!("effect_lowering_stage.rs");
        let builder_source = include_str!("../effect_lowered/builder.rs");
        let production_stage_source = stage_source
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(stage_source);

        for source in [production_stage_source, builder_source] {
            assert!(!source.contains("use crate::llvm"));
            assert!(!source.contains("crate::llvm::"));
            assert!(!source.contains("use crate::effect::state_machine"));
            assert!(!source.contains("crate::effect::state_machine::"));
        }
    }
}
