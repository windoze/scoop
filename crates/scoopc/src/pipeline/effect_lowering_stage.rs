use std::fmt::Write;

use crate::effect_facts::{MaterializedEffectFacts, MirSnapshotBinding};
use crate::effect_lowered::{
    EffectLoweringError, LateLoweredOptOptions, LateLoweredProgram, LateLoweredProgramBuilder,
    run_lir_opt_pipeline,
};
use crate::mir::{MaterializedMir, MaterializedMirPassView};
use crate::ty::TypeStore;
use scoopc_lir_facts::LirFacts;

use super::{EffectFactsStageOutput, MirStageOutput};

/// P5 late-lowering 的显式输入。
///
/// MIR handoff 与 P4 effect facts handoff 必须由调用方分别传入，避免 P5 通过
/// `EffectFactsStageOutput` 回看 P3 输出。
#[derive(Debug)]
pub struct EffectLoweringStageInput {
    mir_stage_output: MirStageOutput,
    effect_facts_stage_output: EffectFactsStageOutput,
}

impl EffectLoweringStageInput {
    pub fn new(
        mir_stage_output: MirStageOutput,
        effect_facts_stage_output: EffectFactsStageOutput,
    ) -> Self {
        Self {
            mir_stage_output,
            effect_facts_stage_output,
        }
    }

    pub fn mir_stage_output(&self) -> &MirStageOutput {
        &self.mir_stage_output
    }
}

/// LIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P5/P6 及后续阶段直接消费：
/// - 输入必须显式区分 P3 的 `MirStageOutput` 与 P4 的 `EffectFactsStageOutput`；
/// - stage 只消费 canonical MIR snapshot + `MaterializedEffectFacts`，不回 HIR/typecheck；
/// - `lir()` / `program()` 返回独立的 `LateLoweredProgram`，它现在是正式 LIR 本体；
/// - `lir_facts()` 返回独立 `scoopc_lir_facts::LirFacts` 数据产品；
/// - 输出不再保存 `EffectFactsStageOutput` 或 `MirStageOutput` wrapper；
/// - 对外暴露的输出不会混有“部分 callable 已 lowered、部分仍停留在 direct-style”的半成品状态；
/// - P6 只应把这份输出翻译到 LLVM，而不是再重做高层 effect lowering 设计；
/// - P6 可消费的 canonical 中层信息包括：`LateLoweredProgram` 内显式区分的 Plain callable
///   ordinary ABI/source slices，以及 EffectStep callable 的 type references、state graph、frame schema、
///   dynamic invoke entry、authoritative per-op/per-schema resume publication（step cases / continuation
///   object / surface-resume dispatch inventory），以及可选的 effect-family resume packing definitions；
/// - P6 明确不得重新做 boundary 识别、whole-function segmentation、frame lifting、
///   continuation capture 合同设计或 `ImplPlan` 选择，也不得把 packing layer 反客为主当成
///   reverse-resume 语义主键；
/// - LLVM 物理布局/ABI/runtime 集成仍属于 TODO-6/P7，而不是在 P5 逆向塞回本阶段。
#[derive(Debug)]
pub struct LirStageOutput {
    lir: LateLoweredProgram,
    lir_facts: LirFacts,
    context: LirStageContext,
}

/// Explicit base context retained for current LLVM/backend compatibility.
///
/// This is deliberately not a nested upstream stage output wrapper. Codegen-neutral
/// contracts live in `LirFacts`; the remaining raw MIR/effect context is a TODO-6/P7
/// backend residual for body emission, reachability, physical layout, and type bridging.
#[derive(Debug)]
struct LirStageContext {
    materialized_mir: MaterializedMir,
    effect_facts: MaterializedEffectFacts,
}

impl LirStageContext {
    fn from_stage_inputs(
        mir_stage_output: MirStageOutput,
        effect_facts_stage_output: EffectFactsStageOutput,
    ) -> Self {
        let (_direct_style, materialized_mir) = mir_stage_output.into_parts();
        Self {
            materialized_mir,
            effect_facts: effect_facts_stage_output.into_effect_facts(),
        }
    }
}

/// Compatibility alias for current callers that still use the old late-lowering name.
pub type EffectLoweredStageOutput = LirStageOutput;

impl LirStageOutput {
    fn new(
        lir: LateLoweredProgram,
        lir_facts: LirFacts,
        mir_stage_output: MirStageOutput,
        effect_facts_stage_output: EffectFactsStageOutput,
    ) -> Self {
        Self {
            lir,
            lir_facts,
            context: LirStageContext::from_stage_inputs(
                mir_stage_output,
                effect_facts_stage_output,
            ),
        }
    }

    fn snapshot_binding(&self) -> &MirSnapshotBinding {
        self.effect_facts().snapshot_binding()
    }

    fn materialized_mir(&self) -> &MaterializedMir {
        &self.context.materialized_mir
    }

    #[cfg_attr(not(feature = "llvm"), allow(dead_code))]
    pub(crate) fn llvm_residual_pass_view(&self) -> MaterializedMirPassView<'_> {
        self.context.materialized_mir.pass_view()
    }

    pub fn types(&self) -> &TypeStore {
        self.effect_facts().types()
    }

    fn effect_facts(&self) -> &MaterializedEffectFacts {
        &self.context.effect_facts
    }

    pub fn lir(&self) -> &LateLoweredProgram {
        &self.lir
    }

    pub fn lir_facts(&self) -> &LirFacts {
        &self.lir_facts
    }

    pub fn program(&self) -> &LateLoweredProgram {
        self.lir()
    }

    /// `dump-effect-lowered` / snapshot / P6 复用的稳定 LIR 文本 surface。
    pub fn stable_dump(&self) -> String {
        render_stage_output(self)
    }

    pub fn into_parts(self) -> (LateLoweredProgram, LirFacts) {
        (self.lir, self.lir_facts)
    }
}

pub(crate) fn run(input: EffectLoweringStageInput) -> Result<LirStageOutput, EffectLoweringError> {
    run_with_opt_options(input, LateLoweredOptOptions::default())
}

#[cfg_attr(not(feature = "llvm"), allow(dead_code))]
pub(crate) fn run_preserving_published_resume_shells(
    input: EffectLoweringStageInput,
) -> Result<LirStageOutput, EffectLoweringError> {
    run_with_opt_options(
        input,
        LateLoweredOptOptions::preserve_published_resume_shells(),
    )
}

#[cfg_attr(not(feature = "llvm"), allow(dead_code))]
fn run_with_opt_options(
    input: EffectLoweringStageInput,
    opt_options: LateLoweredOptOptions,
) -> Result<LirStageOutput, EffectLoweringError> {
    let EffectLoweringStageInput {
        mir_stage_output,
        effect_facts_stage_output,
    } = input;
    let raw_lir = LateLoweredProgramBuilder::from_canonical_inputs(
        mir_stage_output.materialized_pass_view(),
        effect_facts_stage_output.effect_facts(),
        effect_facts_stage_output.effect_facts().types(),
        mir_stage_output.mir_facts(),
    )
    .build()?;
    let (lir, opt_pipeline) = run_lir_opt_pipeline(raw_lir, opt_options)
        .map_err(|error| EffectLoweringError::InvalidLirOptPipelineContract {
            detail: error.to_string(),
        })?
        .into_parts();
    let lir_facts = super::lir_facts_builder::build_lir_facts(
        &lir,
        mir_stage_output.mir_facts(),
        mir_stage_output.materialized_mir(),
        effect_facts_stage_output.effect_facts(),
        mir_stage_output.materialized_mir().opt_level(),
        opt_pipeline,
    )?;
    Ok(LirStageOutput::new(
        lir,
        lir_facts,
        mir_stage_output,
        effect_facts_stage_output,
    ))
}

fn render_stage_output(output: &LirStageOutput) -> String {
    let binding = output.snapshot_binding();
    let mut rendered = String::new();
    writeln!(&mut rendered, "LirStageOutput").unwrap();
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
    writeln!(&mut rendered, "lir_facts:").unwrap();
    rendered.push_str(&output.lir_facts().dump());
    rendered.push('\n');
    writeln!(&mut rendered, "post_opt_lir:").unwrap();
    rendered.push_str(&output.lir().stable_dump());
    rendered
}

#[cfg(test)]
mod tests {
    use super::{EffectLoweringStageInput, LirStageOutput};
    use crate::effect_facts::{CallableAbiKind, CanonicalMirQuerySurface};
    use crate::opt::OptLevel;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use scoopc_lir_facts::{
        LirCallSiteKind, LirCallableContract, LirCallableSymbolKind, LirGlobalRootKind,
        LirGlobalStoragePolicy,
    };

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_lowered_stage_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    fn run_sample() -> LirStageOutput {
        let session = session();
        let source = sample_source();
        super::run(stage_input_for_source(&session, &source))
            .expect("fixture 应可通过 late-lowering stage")
    }

    fn stage_input_for_source(session: &Session, source: &SourceFile) -> EffectLoweringStageInput {
        let mir_stage_output =
            super::super::load_p4_ready_mir_stage_output_for_dump(session, source)
                .expect("fixture 应可通过 P4-ready MIR stage");
        let effect_facts_output =
            super::super::build_effect_facts_stage_output(session, source, &mir_stage_output)
                .expect("fixture 应可通过 effect-facts stage");
        EffectLoweringStageInput::new(mir_stage_output, effect_facts_output)
    }

    fn run_stage_with_opt_level(source: &SourceFile, opt_level: OptLevel) -> LirStageOutput {
        let session = session();
        let materialized =
            crate::mir::materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        let effect_facts_output =
            super::super::build_effect_facts_stage_output(&session, source, &mir_stage_output)
                .unwrap();
        super::run(EffectLoweringStageInput::new(
            mir_stage_output,
            effect_facts_output,
        ))
        .expect("fixture 应可通过 late-lowering stage")
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

    fn global_init_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/top_level_roots.scoop",
            include_str!("../../../../tests/fixtures/mir_lowered/top_level_roots.scoop"),
        )
    }

    fn dispatch_and_resume_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/dispatch_and_resume_call.scoop",
            r#"
package sample

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

class Derived() : Base() {
    override fun ping(): Int {
        return 2
    }
}

interface IFace {
    fun foo(): Int
}

class Impl() : IFace {
    fun foo(): Int {
        return 3
    }
}

fun callVirtual(b: Base): Int {
    return b.ping()
}

fun callInterface(i: IFace): Int {
    return i.foo()
}
"#,
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
        assert_eq!(
            output.lir_facts().summary.callable_count,
            output.program().len()
        );
        assert!(output.program().callable("sample.helper").is_some());
        assert!(output.program().callable("sample.main").is_some());
        assert!(output.stable_dump().contains("LirStageOutput"));
        assert!(output.stable_dump().contains("lir_facts:"));
        assert!(output.stable_dump().contains("LateLoweredProgram"));
    }

    #[test]
    fn effect_lowered_stage_explicitly_consumes_p4_effect_facts_stage_output() {
        let session = session();
        let source = sample_source();
        let mir_stage_output =
            super::super::load_p4_ready_mir_stage_output_for_dump(&session, &source).unwrap();
        let effect_facts_output =
            super::super::build_effect_facts_stage_output(&session, &source, &mir_stage_output)
                .unwrap();
        let main_key = mir_stage_output
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

        let output = super::run(EffectLoweringStageInput::new(
            mir_stage_output,
            effect_facts_output,
        ))
        .unwrap();
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
        let dump = super::run(stage_input_for_source(&session, &source))
            .unwrap()
            .stable_dump();

        assert!(dump.contains("LirStageOutput"));
        assert!(dump.contains("opt_level: O2"));
        assert!(dump.contains("snapshot_binding:"));
        assert!(dump.contains("lir_facts:"));
        assert!(dump.contains("opt_pipeline:"));
        assert!(dump.contains("pass=local-state-machine-elimination"));
        assert!(dump.contains("pass=post-opt-verifier"));
        assert!(dump.contains("post_opt_lir:"));
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
        let dump = super::run(stage_input_for_source(&session, &source))
            .unwrap()
            .stable_dump();

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
        let dump = super::run(stage_input_for_source(&session, &source))
            .unwrap()
            .stable_dump();

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
    fn effect_lowered_lir_facts_publish_dynamic_invoke_and_resume_contracts() {
        let session = session();
        let source = dynamic_fallback_fixture_source();
        let output = super::run(stage_input_for_source(&session, &source)).unwrap();
        let facts = output.lir_facts();

        assert!(!facts.step_types.is_empty());
        assert!(!facts.dynamic_invokes.is_empty());
        assert!(!facts.resume_packings.is_empty());
        assert!(!facts.continuation_objects.is_empty());
        assert!(!facts.surface_resume_dispatches.is_empty());
        let call_value = facts
            .callables
            .values()
            .find(|callable| callable.root_fqn() == "sample.callValue")
            .expect("callValue callable facts should be published");
        let LirCallableContract::EffectStep(effect) = &call_value.contract else {
            panic!("callValue should publish an effect-step ABI contract");
        };
        assert_eq!(effect.step_schema, effect.dynamic_invoke_entry.step_schema);
        assert_eq!(effect.param_tys.len(), 1);
        assert_eq!(effect.closure_carrier_arg_tys, effect.param_tys);
        assert_eq!(effect.control_body.continuation_object.as_u32() as usize, 0);
        assert!(facts.verify().is_ok());
    }

    #[test]
    fn effect_lowered_lir_facts_publish_plain_dispatch_contracts() {
        let session = session();
        let source = dispatch_and_resume_fixture_source();
        let output = super::run(stage_input_for_source(&session, &source)).unwrap();
        let facts = output.lir_facts();

        assert!(facts.dispatches.values().any(|dispatch| {
            dispatch.kind == LirCallSiteKind::Virtual
                && dispatch.owner_fqn == "sample.Base"
                && dispatch.member_name == "ping"
        }));
        assert!(facts.dispatches.values().any(|dispatch| {
            dispatch.kind == LirCallSiteKind::Interface
                && dispatch.owner_fqn == "sample.IFace"
                && dispatch.member_name == "foo"
                && dispatch.interface_id.is_some()
        }));
        let call_virtual = facts
            .callables
            .values()
            .find(|callable| callable.root_fqn() == "sample.callVirtual")
            .expect("callVirtual callable facts should be published");
        let LirCallableContract::Plain(plain) = &call_virtual.contract else {
            panic!("callVirtual should publish a plain ABI contract");
        };
        assert!(plain.call_sites.iter().any(|site| site.dispatch.is_some()));
        assert!(facts.verify().is_ok());
    }

    #[test]
    fn effect_lowered_lir_facts_publish_backend_layout_and_type_contracts() {
        let session = session();
        let source = dispatch_and_resume_fixture_source();
        let output = super::run(stage_input_for_source(&session, &source)).unwrap();
        let facts = output.lir_facts();

        assert!(facts.physical_layout.classes.contains_key("sample.Base"));
        assert!(facts.physical_layout.classes.contains_key("sample.Derived"));
        assert!(
            facts
                .physical_layout
                .class_vtables
                .contains_key("sample.Base")
        );
        assert!(
            facts
                .physical_layout
                .interfaces
                .contains_key("sample.IFace")
        );
        assert!(
            facts
                .physical_layout
                .class_itables
                .contains_key("sample.Impl")
        );
        let call_virtual = facts
            .physical_layout
            .callable_symbols
            .values()
            .find(|symbol| symbol.root_fqn == "sample.callVirtual")
            .expect("callVirtual should publish callable symbol facts");
        assert_eq!(call_virtual.kind, LirCallableSymbolKind::ManagedOrdinary);
        assert_eq!(call_virtual.param_tys.len(), 1);
        assert!(!facts.type_context.primary_fingerprint.is_empty());
        assert!(!facts.type_context.stable_wire_format.owner.is_empty());
        let dump = output.stable_dump();
        assert!(dump.contains("physical_layout:"));
        assert!(dump.contains("type_context:"));
        assert!(facts.verify().is_ok());
    }

    #[test]
    fn lir_facts_builder_publishes_global_init_storage_contracts() {
        let session = session();
        let source = global_init_fixture_source();
        let output = super::run(stage_input_for_source(&session, &source)).unwrap();
        let facts = output.lir_facts();
        let global_init = &facts.global_init;

        assert_eq!(global_init.roots.len(), 5);
        assert_eq!(global_init.top_level_eager_inits.len(), 3);
        assert_eq!(global_init.object_once.len(), 1);
        assert_eq!(global_init.cone_init_routines.len(), 1);
        assert_eq!(global_init.final_entry_order.routines.len(), 1);

        let counter = global_init
            .roots
            .values()
            .find(|root| root.root.as_str() == "mir_lowered.top_level_roots.Counter")
            .expect("Counter root should be published");
        assert_eq!(counter.kind, LirGlobalRootKind::TopLevelMutableVar);
        assert_eq!(counter.storage, Some(LirGlobalStoragePolicy::Global));
        assert_eq!(counter.dependencies.len(), 1);
        let counter_body = counter
            .initializer_body
            .as_ref()
            .expect("Counter eager init should publish source/body contract");
        assert_eq!(counter_body.root.as_str(), counter.root.as_str());
        assert_eq!(counter_body.body_item_count, 1);
        assert_eq!(
            counter.dependencies[0].target.as_str(),
            "mir_lowered.top_level_roots.Runtime"
        );

        let native = global_init
            .roots
            .values()
            .find(|root| root.root.as_str() == "mir_lowered.top_level_roots.NativeCounter")
            .expect("extern global root should be published");
        assert_eq!(native.kind, LirGlobalRootKind::ExternGlobal);
        assert_eq!(native.storage, Some(LirGlobalStoragePolicy::Global));
        assert!(!native.has_initializer);
        assert_eq!(
            native
                .extern_global
                .as_ref()
                .map(|global| global.symbol.as_str()),
            Some("native_counter")
        );

        let routine = global_init
            .cone_init_routines
            .values()
            .next()
            .expect("per-cone init routine should be published");
        let ordered_roots = routine
            .roots
            .iter()
            .map(|root| root.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ordered_roots,
            vec![
                "mir_lowered.top_level_roots.Base",
                "mir_lowered.top_level_roots.Runtime",
                "mir_lowered.top_level_roots.Counter",
            ]
        );
        let dump = output.stable_dump();
        assert!(dump.contains("global_init: roots=5 object_once=1 top_level_eager_inits=3"));
        assert!(dump.contains("extern_symbol=native_counter"));
        assert!(facts.verify().is_ok());
    }

    #[test]
    fn effect_lowered_stage_dump_exposes_handle_and_resume_site_authoritative_sources() {
        let session = session();
        let source = local_runtime_error_fixture_source();
        let dump = super::run(stage_input_for_source(&session, &source))
            .unwrap()
            .stable_dump();

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
