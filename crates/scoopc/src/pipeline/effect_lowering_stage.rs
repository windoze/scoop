use std::fmt::Write;

use crate::effect_facts::{MaterializedEffectFacts, MirSnapshotBinding};
use crate::effect_lowered::{
    EffectLoweringError, LateLoweredOptOptions, LateLoweredProgram, LateLoweredProgramBuilder,
    optimize_program, optimize_program_with_options,
};
use crate::mir::{MaterializedMir, MaterializedMirPassView};
use crate::ty::TypeStore;

use super::EffectFactsStageOutput;

/// late-lowering stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P5/P6 及后续阶段直接消费：
/// - 输入必须是 P4 的 `EffectFactsStageOutput`；
/// - stage 只消费 canonical MIR snapshot + `MaterializedEffectFacts`，不回 HIR/typecheck；
/// - `program()` 返回独立的 `LateLoweredProgram`，后续结构性 rewrite 必须继续在这份
///   late-lowered IR 上工作，而不是回头 patch P3/P4 产物；
/// - 对外暴露的输出不会混有“部分 callable 已 lowered、部分仍停留在 direct-style”的半成品状态；
/// - P6 只应把这份输出翻译到 LLVM，而不是再重做高层 effect lowering 设计；
/// - P6 可消费的 canonical 中层信息包括：`LateLoweredProgram` 内显式区分的 Plain callable
///   ordinary ABI/source slices，以及 EffectStep callable 的 type references、state graph、frame schema、
///   dynamic invoke entry、authoritative per-op/per-schema resume publication（step cases / continuation
///   object / surface-resume dispatch inventory），以及可选的 effect-family resume packing definitions；
/// - P6 明确不得重新做 boundary 识别、whole-function segmentation、frame lifting、
///   continuation capture 合同设计或 `ImplPlan` 选择，也不得把 packing layer 反客为主当成
///   reverse-resume 语义主键；
/// - LLVM 物理布局/ABI/runtime 集成仍属于 P6，而不是在 P5 逆向塞回本阶段。
#[derive(Debug)]
pub struct EffectLoweredStageOutput {
    effect_facts_stage_output: EffectFactsStageOutput,
    program: LateLoweredProgram,
}

impl EffectLoweredStageOutput {
    fn new(effect_facts_stage_output: EffectFactsStageOutput, program: LateLoweredProgram) -> Self {
        Self {
            effect_facts_stage_output,
            program,
        }
    }

    pub fn effect_facts_stage_output(&self) -> &EffectFactsStageOutput {
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

    pub fn into_parts(self) -> (EffectFactsStageOutput, LateLoweredProgram) {
        (self.effect_facts_stage_output, self.program)
    }
}

pub(crate) fn run(
    effect_facts_stage_output: EffectFactsStageOutput,
) -> Result<EffectLoweredStageOutput, EffectLoweringError> {
    let nominal_direct_supertypes =
        crate::effect_lowered::builder::collect_nominal_direct_supertypes_from_mir_file(
            effect_facts_stage_output.file(),
        );
    let program = optimize_program(
        LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .with_nominal_direct_supertypes(nominal_direct_supertypes)
        .build()?,
    );
    Ok(EffectLoweredStageOutput::new(
        effect_facts_stage_output,
        program,
    ))
}

#[cfg_attr(not(feature = "llvm"), allow(dead_code))]
pub(crate) fn run_preserving_published_resume_shells(
    effect_facts_stage_output: EffectFactsStageOutput,
) -> Result<EffectLoweredStageOutput, EffectLoweringError> {
    run_with_opt_options(
        effect_facts_stage_output,
        LateLoweredOptOptions::preserve_published_resume_shells(),
    )
}

#[cfg_attr(not(feature = "llvm"), allow(dead_code))]
fn run_with_opt_options(
    effect_facts_stage_output: EffectFactsStageOutput,
    opt_options: LateLoweredOptOptions,
) -> Result<EffectLoweredStageOutput, EffectLoweringError> {
    let nominal_direct_supertypes =
        crate::effect_lowered::builder::collect_nominal_direct_supertypes_from_mir_file(
            effect_facts_stage_output.file(),
        );
    let program = optimize_program_with_options(
        LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .with_nominal_direct_supertypes(nominal_direct_supertypes)
        .build()?,
        opt_options,
    );
    Ok(EffectLoweredStageOutput::new(
        effect_facts_stage_output,
        program,
    ))
}

fn render_stage_output(output: &EffectLoweredStageOutput) -> String {
    let binding = output.snapshot_binding();
    let mut rendered = String::new();
    writeln!(&mut rendered, "EffectLoweredStageOutput").unwrap();
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
    use super::EffectLoweredStageOutput;
    use crate::effect_facts::{CallableAbiKind, CanonicalMirQuerySurface};
    use crate::opt::OptLevel;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_lowered_stage_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    fn run_sample() -> EffectLoweredStageOutput {
        let session = session();
        let source = sample_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        super::run(effect_facts_output).expect("fixture 应可通过 late-lowering stage")
    }

    fn run_stage_with_opt_level(
        source: &SourceFile,
        opt_level: OptLevel,
    ) -> EffectLoweredStageOutput {
        let session = session();
        let materialized =
            crate::mir::materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        let effect_facts_output =
            super::super::build_effect_facts_stage_output(&session, source, mir_stage_output)
                .unwrap();
        super::run(effect_facts_output).expect("fixture 应可通过 late-lowering stage")
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

    fn local_runtime_error_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_resume_if_else_branch_single_perform.scoop",
            include_str!(
                "../../../../tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop"
            ),
        )
    }

    fn dynamic_fallback_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/dynamic_fallback_widening.scoop",
            include_str!(
                "../../../../tests/fixtures/effect_lowered/dynamic_fallback_widening.scoop"
            ),
        )
    }

    fn handle_finally_boundary_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/handle_finally_boundary.scoop",
            include_str!("../../../../tests/fixtures/mir_lowered/handle_finally_boundary.scoop"),
        )
    }

    #[test]
    fn effect_lowered_stage_output_is_constructible() {
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
        assert!(output.stable_dump().contains("EffectLoweredStageOutput"));
        assert!(output.stable_dump().contains("LateLoweredProgram"));
    }

    #[test]
    fn effect_lowered_stage_explicitly_consumes_p4_effect_facts_stage_output() {
        let session = session();
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
        let expected_impl_plan = main_facts.impl_plan();
        let expected_needs_reentry = main_facts.needs_reentry();
        let expected_cases = main_facts.resolved_outward_cases().tags().to_vec();
        let expected_abi = main_facts.call_abi_kind();
        let expected_body_step_schema = main_facts.body_step_schema();

        let output = super::run(effect_facts_output).unwrap();
        let lowered_main = output
            .program()
            .callable("sample.main")
            .expect("late-lowered program 应保留 sample.main 边界记录");

        assert_eq!(lowered_main.call_abi_kind(), expected_abi);
        assert_eq!(lowered_main.body_step_schema(), expected_body_step_schema);
        assert_eq!(lowered_main.impl_plan(), expected_impl_plan);
        assert_eq!(lowered_main.needs_reentry(), expected_needs_reentry);
        assert_eq!(
            lowered_main.resolved_outward_cases(),
            expected_cases.as_slice()
        );
        assert_eq!(
            lowered_main.plain_abi().is_some(),
            expected_abi == CallableAbiKind::Plain
        );
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.program().len()
        );
    }

    #[test]
    fn effect_lowered_no_outward_plain_callable_handoff() {
        let output = run_sample();
        let main = output
            .program()
            .callable("sample.main")
            .expect("late-lowered program 应保留 sample.main");

        assert_eq!(main.call_abi_kind(), CallableAbiKind::Plain);
        assert_eq!(main.impl_plan(), crate::effect_facts::ImplPlan::NoOutward);
        assert!(main.body_step_schema().is_none());
        assert!(main.effect_step_abi().is_none());
        let plain = main
            .plain_abi()
            .expect("NoOutward callable 应发布 plain ABI handoff");
        assert_eq!(plain.param_tys().len(), 0);
        assert!(!plain.body_slices().is_empty());
        assert!(output.program().step_types().is_empty());
        assert!(output.program().continuation_objects().is_empty());
    }

    #[test]
    fn effect_lowered_stage_stable_dump_lists_post_opt_late_lowered_sections() {
        let session = session();
        let source = dump_fixture_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let dump = super::run(effect_facts_output).unwrap().stable_dump();

        assert!(dump.contains("EffectLoweredStageOutput"));
        assert!(dump.contains("opt_level: O2"));
        assert!(dump.contains("snapshot_binding:"));
        assert!(dump.contains("post_opt_program:"));
        assert!(dump.contains("LateLoweredProgram"));
        assert!(dump.contains("step_types:"));
        assert!(dump.contains("continuation_objects:"));
        assert!(dump.contains("authoritative_surface_resume_dispatch_inventory:"));
        assert!(dump.contains("resume_packing_interfaces:"));
        assert!(dump.contains("callables:"));
        assert!(dump.contains("state_graph:"));
        assert!(dump.contains("frame_schema:"));
        assert!(dump.contains("boundary_map:"));
        assert!(dump.contains("resume_state_map:"));
        assert!(dump.contains("cleanup_state:"));
        assert!(dump.contains("drop_state:"));
    }

    #[test]
    fn effect_lowered_stage_stable_dump_locks_opt_level_visible_impl_plan() {
        let source = single_case_source();
        let o0 = run_stage_with_opt_level(&source, OptLevel::O0).stable_dump();
        let o2 = run_stage_with_opt_level(&source, OptLevel::O2).stable_dump();

        assert!(o0.contains("opt_level: O0"));
        assert!(o0.contains("impl_plan=CanonicalFull"));
        assert!(o2.contains("opt_level: O2"));
        assert!(o2.contains("impl_plan=SingleCase("));
        assert!(o2.contains("sample.Ping.hit"));
    }

    #[test]
    fn effect_lowered_stage_dump_exposes_plain_effect_step_call_contract() {
        let session = session();
        let source = local_runtime_error_fixture_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let dump = super::run(effect_facts_output).unwrap().stable_dump();

        assert!(dump.contains("root: main"));
        assert!(dump.contains("abi: Plain"));
        assert!(dump.contains("plain_call_sites:"));
        assert!(dump.contains("target=executeCase callee_abi=EffectStep"));
        assert!(dump.contains("callee_step_schema=step#h"));
        assert!(dump.contains("resolved_cases=[case#h"));
        assert!(dump.contains("dispatch=EffectStepDispatch"));
        assert!(dump.contains("plain_local_effect_control: step#h"));
        assert!(dump.contains("consumed_runtime_error_case: in case#h"));
        assert!(dump.contains("scoop.core.Raise.raise"));
    }

    #[test]
    fn effect_lowered_stage_dump_prioritizes_authoritative_surface_resume_dispatch() {
        let session = session();
        let source = dynamic_fallback_fixture_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let dump = super::run(effect_facts_output).unwrap().stable_dump();

        let dispatch_pos = dump
            .find("authoritative_surface_resume_dispatch_inventory:")
            .expect("stable dump 应显式列出 authoritative surface-resume inventory");
        let packing_pos = dump
            .find("resume_packing_interfaces:")
            .expect("stable dump 应显式保留 packing layer");

        assert!(
            dispatch_pos < packing_pos,
            "authoritative dispatch contract 应先于 packing layer 出现\n{dump}"
        );
        for needle in [
            "continuation_schema: cont#h",
            "source=ContinuationObjectMethod",
            "surface_case cont_obj#h",
            "internal_method cont_obj#h",
            "packed_by=packing#h",
            "resume_packing_interface: packing#h",
        ] {
            assert!(
                dump.contains(needle),
                "dynamic fallback dump 应直接暴露 authoritative per-op/per-schema contract: {needle}\n{dump}"
            );
        }
    }

    #[test]
    fn effect_lowered_stage_dump_exposes_handle_and_resume_site_authoritative_sources() {
        let session = session();
        let source = local_runtime_error_fixture_source();
        let effect_facts_output =
            super::super::load_effect_facts_stage_output_for_dump(&session, &source).unwrap();
        let dump = super::run(effect_facts_output).unwrap().stable_dump();

        for needle in [
            "continuation_schema: cont#h",
            "source=HandleContinuationBinderOnly",
            "handle_continuation_binder instance=executeCase allowed_row=Pure impl_plan=SingleCase(",
            "cont_obj#h",
            "site#h",
            "arm#0 handled_case=case#h",
            "source=OwnerTrampolineMixed",
            "resume_boundary instance=executeCase allowed_row=Pure impl_plan=SingleCase(",
        ] {
            assert!(
                dump.contains(needle),
                "run-pass fixture dump 应直接暴露 non-object authoritative dispatch source: {needle}\n{dump}"
            );
        }
    }

    #[test]
    fn mir_policy_gates_dump_resume_unwind_pending_completion_contract() {
        let source = handle_finally_boundary_fixture_source();
        let dump = run_stage_with_opt_level(&source, OptLevel::O0).stable_dump();

        for needle in [
            "handle_contract:",
            "pending_completions:",
            "ContinueToExit",
            "ReturnFromFunction",
            "pending_completion_origins:",
            "pending_payload_transports:",
            "ResumeUnwind",
        ] {
            assert!(
                dump.contains(needle),
                "policy gate dump should expose cleanup/finally pending completion contract: {needle}\n{dump}"
            );
        }
    }
}
