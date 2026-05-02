use std::fmt::Write;

use crate::effect_facts::{MaterializedEffectFacts, MirSnapshotBinding};
use crate::effect_lowered::{
    EffectLoweringError, LateLoweredOptOptions, LateLoweredProgram, LateLoweredProgramBuilder,
    optimize_program, optimize_program_with_options,
};
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
/// - P6 只应把这份输出翻译到 LLVM，而不是再重做高层 effect lowering 设计；
/// - P6 可消费的 canonical 中层信息包括：`LateLoweredProgram` 内的 type references、
///   state graph、frame schema、dynamic invoke entry、resume interface definitions 与
///   continuation object definitions；
/// - P6 明确不得重新做 boundary 识别、whole-function segmentation、frame lifting、
///   continuation capture 合同设计或 `ImplPlan` 选择；
/// - LLVM 物理布局/ABI/runtime 集成仍属于 P6，而不是在 P5 逆向塞回本阶段。
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
    let program = optimize_program(
        LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .build()?,
    );
    Ok(RefactorEffectLoweredStageOutput::new(
        effect_facts_stage_output,
        program,
    ))
}

pub(crate) fn run_preserving_published_resume_shells(
    effect_facts_stage_output: RefactorEffectFactsStageOutput,
) -> Result<RefactorEffectLoweredStageOutput, EffectLoweringError> {
    run_with_opt_options(
        effect_facts_stage_output,
        LateLoweredOptOptions::preserve_published_resume_shells(),
    )
}

fn run_with_opt_options(
    effect_facts_stage_output: RefactorEffectFactsStageOutput,
    opt_options: LateLoweredOptOptions,
) -> Result<RefactorEffectLoweredStageOutput, EffectLoweringError> {
    let program = optimize_program_with_options(
        LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .build()?,
        opt_options,
    );
    Ok(RefactorEffectLoweredStageOutput::new(
        effect_facts_stage_output,
        program,
    ))
}

fn render_stage_output(output: &RefactorEffectLoweredStageOutput) -> String {
    let binding = output.snapshot_binding();
    let mut rendered = String::new();
    writeln!(&mut rendered, "RefactorEffectLoweredStageOutput").unwrap();
    writeln!(
        &mut rendered,
        "opt_level: O{}",
        output.materialized_mir().opt_level().as_str()
    )
    .unwrap();
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
    writeln!(&mut rendered, "post_opt_program:").unwrap();
    rendered.push_str(&output.program().stable_dump());
    rendered
}

#[cfg(test)]
mod tests {
    use super::RefactorEffectLoweredStageOutput;
    use crate::effect_facts::CanonicalMirQuerySurface;
    use crate::opt::OptLevel;
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

    fn run_stage_with_opt_level(
        source: &SourceFile,
        opt_level: OptLevel,
    ) -> RefactorEffectLoweredStageOutput {
        let session = refactor_session();
        let materialized =
            crate::mir::materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        let effect_facts_output =
            super::super::build_effect_facts_stage_output(&session, source, mir_stage_output)
                .unwrap();
        super::run(effect_facts_output).expect("fixture 应可通过 refactor late-lowering stage")
    }

    fn dump_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_lowered_dump_fixture.scoop",
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

    fn single_case_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_lowered_single_case_fixture.scoop",
            r#"
package sample

effect Ping {
    fun hit(): Unit
}

fun leaf(): Unit / Ping {
    Ping.hit()
}
"#,
        )
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
    fn refactor_effect_lowered_stage_stable_dump_lists_post_opt_late_lowered_sections() {
        let session = refactor_session();
        let source = dump_fixture_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let dump = super::run(effect_facts_output).unwrap().stable_dump();

        assert!(dump.contains("RefactorEffectLoweredStageOutput"));
        assert!(dump.contains("opt_level: O2"));
        assert!(dump.contains("snapshot_binding:"));
        assert!(dump.contains("post_opt_program:"));
        assert!(dump.contains("LateLoweredProgram"));
        assert!(dump.contains("step_types:"));
        assert!(dump.contains("resume_interfaces:"));
        assert!(dump.contains("continuation_objects:"));
        assert!(dump.contains("callables:"));
        assert!(dump.contains("state_graph:"));
        assert!(dump.contains("frame_schema:"));
        assert!(dump.contains("boundary_map:"));
        assert!(dump.contains("resume_state_map:"));
        assert!(dump.contains("cleanup_state:"));
        assert!(dump.contains("drop_state:"));
    }

    #[test]
    fn refactor_effect_lowered_stage_stable_dump_locks_opt_level_visible_impl_plan() {
        let source = single_case_source();
        let o0 = run_stage_with_opt_level(&source, OptLevel::O0).stable_dump();
        let o2 = run_stage_with_opt_level(&source, OptLevel::O2).stable_dump();

        assert!(o0.contains("opt_level: O0"));
        assert!(o0.contains("impl_plan=CanonicalFull"));
        assert!(o2.contains("opt_level: O2"));
        assert!(o2.contains("impl_plan=SingleCase(c0)"));
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
