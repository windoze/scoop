use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use crate::effect_lowered::{
    EffectLoweringError, LateLoweredOptOptions, LateLoweredProgram, LateLoweredProgramBuilder,
    run_lir_opt_pipeline,
};
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
}

impl LirStageOutput {
    fn new(lir: LateLoweredProgram, lir_facts: LirFacts) -> Self {
        Self { lir, lir_facts }
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
fn run_with_opt_options(
    input: EffectLoweringStageInput,
    opt_options: LateLoweredOptOptions,
) -> Result<LirStageOutput, EffectLoweringError> {
    let EffectLoweringStageInput {
        mir_stage_output,
        effect_facts_stage_output,
    } = input;
    build_lir_stage_output_from_stage_outputs(
        &mir_stage_output,
        &effect_facts_stage_output,
        opt_options,
    )
}

#[cfg_attr(not(feature = "llvm"), allow(dead_code))]
pub(crate) fn build_lir_stage_output_from_stage_outputs(
    mir_stage_output: &MirStageOutput,
    effect_facts_stage_output: &EffectFactsStageOutput,
    opt_options: LateLoweredOptOptions,
) -> Result<LirStageOutput, EffectLoweringError> {
    let effect_facts =
        convert_stage_effect_facts_to_lir(mir_stage_output, effect_facts_stage_output);
    let raw_lir = LateLoweredProgramBuilder::from_canonical_inputs(
        mir_stage_output.materialized_pass_view(),
        &effect_facts,
        effect_facts.types(),
        mir_stage_output.mir_facts(),
    )
    .build()?;
    let (lir, opt_pipeline) = run_lir_opt_pipeline(raw_lir, opt_options)
        .map_err(|error| EffectLoweringError::InvalidLirOptPipelineContract {
            detail: error.to_string(),
        })?
        .into_parts();
    let lir = super::lir_facts_builder::attach_lir_identity(lir, effect_facts.types())?;
    let lir_facts = super::lir_facts_builder::build_lir_facts(
        &lir,
        mir_stage_output
            .hir_semantic_artifact()
            .map(|artifact| artifact.hir_facts()),
        mir_stage_output.mir_facts(),
        mir_stage_output.materialized_mir(),
        &effect_facts,
        mir_stage_output.materialized_mir().opt_level(),
        opt_pipeline,
    )?;
    Ok(LirStageOutput::new(lir, lir_facts))
}

fn convert_stage_effect_facts_to_lir(
    mir_stage_output: &MirStageOutput,
    effect_facts_stage_output: &EffectFactsStageOutput,
) -> scoopc_lir::effect_facts::MaterializedEffectFacts {
    let facts = effect_facts_stage_output.effect_facts();
    scoopc_lir::effect_facts::MaterializedEffectFacts::new(
        scoopc_lir::effect_facts::EffectOwnedTypeContext::from_types(
            effect_facts_stage_output.effect_types().clone(),
        ),
        scoopc_lir::effect_facts::MirSnapshotBinding::from_pass_view(
            &mir_stage_output.materialized_pass_view(),
        ),
        facts
            .step_schemas()
            .iter()
            .map(|(id, schema)| (map_step_schema_id(*id), map_step_schema(schema)))
            .collect::<BTreeMap<_, _>>(),
        facts
            .continuation_schemas()
            .iter()
            .map(|(id, schema)| {
                (
                    map_continuation_schema_id(*id),
                    map_continuation_schema(schema),
                )
            })
            .collect::<BTreeMap<_, _>>(),
        facts
            .callable_facts()
            .iter()
            .map(|(instance, callable)| (instance.clone(), map_callable_facts(callable)))
            .collect::<HashMap<_, _>>(),
        facts
            .bodies()
            .iter()
            .map(|(instance, body)| (instance.clone(), map_body_facts(body)))
            .collect::<HashMap<_, _>>(),
    )
}

fn map_step_schema(
    schema: &crate::effect_facts_stage::StepSchema,
) -> scoopc_lir::effect_facts::StepSchema {
    scoopc_lir::effect_facts::StepSchema::new(
        schema.invoke_args_tuple_ty(),
        schema.complete_ty(),
        schema.continuation_obj_ty(),
        schema.cases().iter().map(map_step_case).collect(),
    )
}

fn map_step_case(
    case: &crate::effect_facts_stage::StepCaseFact,
) -> scoopc_lir::effect_facts::StepCaseFact {
    scoopc_lir::effect_facts::StepCaseFact::new(
        map_case_tag(case.case_tag()),
        map_concrete_op_key(case.concrete_op_key()),
        case.payload_tuple_ty(),
        map_continuation_schema_id(case.continuation_schema()),
    )
}

fn map_concrete_op_key(
    key: &crate::effect_facts_stage::ConcreteOpKey,
) -> scoopc_lir::effect_facts::ConcreteOpKey {
    scoopc_lir::effect_facts::ConcreteOpKey::new(
        key.instance_key().clone(),
        key.stable_instance_key().clone(),
        map_effect_family_key(key.effect_family()),
    )
}

fn map_effect_family_key(
    key: &crate::effect_facts_stage::EffectFamilyKey,
) -> scoopc_lir::effect_facts::EffectFamilyKey {
    scoopc_lir::effect_facts::EffectFamilyKey::new(
        key.effect_fqn().to_string(),
        key.type_args().to_vec(),
    )
}

fn map_continuation_schema(
    schema: &crate::effect_facts_stage::ContinuationSchema,
) -> scoopc_lir::effect_facts::ContinuationSchema {
    scoopc_lir::effect_facts::ContinuationSchema::new(
        schema.resume_tuple_ty(),
        schema.answer_ty(),
        map_step_schema_id(schema.out_step_schema()),
        schema.surface_ty(),
    )
}

fn map_callable_facts(
    facts: &crate::effect_facts_stage::CallableEffectFacts,
) -> scoopc_lir::effect_facts::CallableEffectFacts {
    scoopc_lir::effect_facts::CallableEffectFacts::new(
        facts.declared_row().clone(),
        map_callable_abi(facts.call_abi_kind()),
        facts.invoke_args_tuple_ty_opt(),
        facts.body_step_schema().map(map_step_schema_id),
        map_case_set(facts.resolved_outward_cases()),
        facts.needs_reentry(),
        map_impl_plan(facts.impl_plan()),
    )
}

fn map_body_facts(
    body: &crate::effect_facts_stage::BodyEffectFacts,
) -> scoopc_lir::effect_facts::BodyEffectFacts {
    let blocks = body
        .blocks()
        .iter()
        .map(|(block_id, block)| (*block_id, map_block_facts(block)))
        .collect();
    let sites = body
        .sites()
        .iter()
        .map(|(site_id, site)| (*site_id, map_site_facts(site)))
        .collect();
    scoopc_lir::effect_facts::BodyEffectFacts::with_local_control_step_schema(
        blocks,
        sites,
        body.local_control_step_schema().map(map_step_schema_id),
    )
}

fn map_block_facts(
    block: &crate::effect_facts_stage::BlockEffectFacts,
) -> scoopc_lir::effect_facts::BlockEffectFacts {
    scoopc_lir::effect_facts::BlockEffectFacts::new(
        map_case_set(block.ambient_cases()),
        map_case_set(block.outward_cases()),
        block.has_suspend_boundary(),
        block.has_handle_boundary(),
    )
}

fn map_site_facts(
    site: &crate::effect_facts_stage::SiteEffectFacts,
) -> scoopc_lir::effect_facts::SiteEffectFacts {
    match site {
        crate::effect_facts_stage::SiteEffectFacts::Call(call) => {
            scoopc_lir::effect_facts::SiteEffectFacts::Call(map_call_site(call))
        }
        crate::effect_facts_stage::SiteEffectFacts::ClassCtor(class_ctor) => {
            scoopc_lir::effect_facts::SiteEffectFacts::ClassCtor(
                scoopc_lir::effect_facts::ClassCtorSiteEffectFacts::new(map_case_set(
                    class_ctor.emitted_cases(),
                )),
            )
        }
        crate::effect_facts_stage::SiteEffectFacts::Perform(perform) => {
            scoopc_lir::effect_facts::SiteEffectFacts::Perform(
                scoopc_lir::effect_facts::PerformSiteEffectFacts::new(
                    map_case_tag(perform.emitted_case()),
                    perform.payload_tuple_ty(),
                    map_continuation_schema_id(perform.captured_cont_schema()),
                ),
            )
        }
        crate::effect_facts_stage::SiteEffectFacts::Resume(resume) => {
            scoopc_lir::effect_facts::SiteEffectFacts::Resume(
                scoopc_lir::effect_facts::ResumeSiteEffectFacts::new(
                    map_continuation_schema_id(resume.continuation_schema()),
                    resume.resume_tuple_ty(),
                    resume.answer_ty(),
                    map_step_schema_id(resume.out_step_schema()),
                    map_case_set(resume.resolved_cases()),
                ),
            )
        }
        crate::effect_facts_stage::SiteEffectFacts::Handle(handle) => {
            scoopc_lir::effect_facts::SiteEffectFacts::Handle(
                scoopc_lir::effect_facts::HandleSiteEffectFacts::new(
                    handle.result_ty(),
                    map_case_set(handle.handled_cases()),
                    map_case_set(handle.body_outward_cases()),
                    handle.arm_facts().iter().map(map_handle_arm).collect(),
                    map_case_set(handle.finally_outward_cases()),
                    map_nested_handle_classification(handle.nested_handle_classification()),
                ),
            )
        }
    }
}

fn map_call_site(
    call: &crate::effect_facts_stage::CallSiteEffectFacts,
) -> scoopc_lir::effect_facts::CallSiteEffectFacts {
    scoopc_lir::effect_facts::CallSiteEffectFacts::new_with_abi(
        map_call_site_kind(call.kind()),
        map_call_site_target(call.target()),
        map_callable_abi(call.callee_abi_kind()),
        call.invoke_args_tuple_ty(),
        call.callee_step_schema().map(map_step_schema_id),
        map_case_set(call.resolved_cases()),
        map_effect_precision(call.precision()),
    )
}

fn map_call_site_target(
    target: &crate::effect_facts_stage::CallSiteTarget,
) -> scoopc_lir::effect_facts::CallSiteTarget {
    match target {
        crate::effect_facts_stage::CallSiteTarget::KnownInstance(instance) => {
            scoopc_lir::effect_facts::CallSiteTarget::KnownInstance(instance.clone())
        }
        crate::effect_facts_stage::CallSiteTarget::CandidateSet(instances) => {
            scoopc_lir::effect_facts::CallSiteTarget::CandidateSet(instances.clone())
        }
        crate::effect_facts_stage::CallSiteTarget::BodylessDirect { fqn } => {
            scoopc_lir::effect_facts::CallSiteTarget::BodylessDirect { fqn: fqn.clone() }
        }
        crate::effect_facts_stage::CallSiteTarget::DynamicFallback => {
            scoopc_lir::effect_facts::CallSiteTarget::DynamicFallback
        }
    }
}

fn map_handle_arm(
    arm: &crate::effect_facts_stage::HandleArmEffectFacts,
) -> scoopc_lir::effect_facts::HandleArmEffectFacts {
    scoopc_lir::effect_facts::HandleArmEffectFacts::new(
        map_case_tag(arm.handled_case()),
        arm.payload_tuple_ty(),
        map_continuation_schema_id(arm.continuation_schema()),
        map_case_set(arm.arm_outward_cases()),
    )
}

fn map_case_set(
    case_set: &crate::effect_facts_stage::CaseSet,
) -> scoopc_lir::effect_facts::CaseSet {
    scoopc_lir::effect_facts::CaseSet::new(
        map_step_schema_id(case_set.schema()),
        case_set.tags().iter().copied().map(map_case_tag).collect(),
    )
}

fn map_step_schema_id(
    id: crate::effect_facts_stage::StepSchemaId,
) -> scoopc_lir::effect_facts::StepSchemaId {
    scoopc_lir::effect_facts::StepSchemaId::new(id.as_u32())
}

fn map_continuation_schema_id(
    id: crate::effect_facts_stage::ContinuationSchemaId,
) -> scoopc_lir::effect_facts::ContinuationSchemaId {
    scoopc_lir::effect_facts::ContinuationSchemaId::new(id.as_u32())
}

fn map_case_tag(tag: crate::effect_facts_stage::CaseTag) -> scoopc_lir::effect_facts::CaseTag {
    scoopc_lir::effect_facts::CaseTag::new(tag.as_u32())
}

fn map_callable_abi(
    kind: crate::effect_facts_stage::CallableAbiKind,
) -> scoopc_lir::effect_facts::CallableAbiKind {
    match kind {
        crate::effect_facts_stage::CallableAbiKind::Plain => {
            scoopc_lir::effect_facts::CallableAbiKind::Plain
        }
        crate::effect_facts_stage::CallableAbiKind::EffectStep => {
            scoopc_lir::effect_facts::CallableAbiKind::EffectStep
        }
    }
}

fn map_impl_plan(plan: crate::effect_facts_stage::ImplPlan) -> scoopc_lir::effect_facts::ImplPlan {
    match plan {
        crate::effect_facts_stage::ImplPlan::NoOutward => {
            scoopc_lir::effect_facts::ImplPlan::NoOutward
        }
        crate::effect_facts_stage::ImplPlan::SingleCase(case) => {
            scoopc_lir::effect_facts::ImplPlan::SingleCase(map_case_tag(case))
        }
        crate::effect_facts_stage::ImplPlan::CanonicalFull => {
            scoopc_lir::effect_facts::ImplPlan::CanonicalFull
        }
    }
}

fn map_call_site_kind(
    kind: crate::effect_facts_stage::CallSiteKind,
) -> scoopc_lir::effect_facts::CallSiteKind {
    match kind {
        crate::effect_facts_stage::CallSiteKind::Direct => {
            scoopc_lir::effect_facts::CallSiteKind::Direct
        }
        crate::effect_facts_stage::CallSiteKind::Closure => {
            scoopc_lir::effect_facts::CallSiteKind::Closure
        }
        crate::effect_facts_stage::CallSiteKind::FunValue => {
            scoopc_lir::effect_facts::CallSiteKind::FunValue
        }
        crate::effect_facts_stage::CallSiteKind::FunPtr => {
            scoopc_lir::effect_facts::CallSiteKind::FunPtr
        }
        crate::effect_facts_stage::CallSiteKind::Virtual => {
            scoopc_lir::effect_facts::CallSiteKind::Virtual
        }
        crate::effect_facts_stage::CallSiteKind::Interface => {
            scoopc_lir::effect_facts::CallSiteKind::Interface
        }
    }
}

fn map_effect_precision(
    precision: crate::effect_facts_stage::EffectPrecision,
) -> scoopc_lir::effect_facts::EffectPrecision {
    match precision {
        crate::effect_facts_stage::EffectPrecision::Precise => {
            scoopc_lir::effect_facts::EffectPrecision::Precise
        }
        crate::effect_facts_stage::EffectPrecision::Widened => {
            scoopc_lir::effect_facts::EffectPrecision::Widened
        }
        crate::effect_facts_stage::EffectPrecision::SignatureFallback => {
            scoopc_lir::effect_facts::EffectPrecision::SignatureFallback
        }
    }
}

fn map_nested_handle_classification(
    classification: crate::effect_facts_stage::NestedHandleClassification,
) -> scoopc_lir::effect_facts::NestedHandleClassification {
    match classification {
        crate::effect_facts_stage::NestedHandleClassification::SelfContained => {
            scoopc_lir::effect_facts::NestedHandleClassification::SelfContained
        }
        crate::effect_facts_stage::NestedHandleClassification::MaySuspendOutward => {
            scoopc_lir::effect_facts::NestedHandleClassification::MaySuspendOutward
        }
    }
}

fn render_stage_output(output: &LirStageOutput) -> String {
    let mut rendered = String::new();
    writeln!(&mut rendered, "LirStageOutput").unwrap();
    writeln!(
        &mut rendered,
        "opt_level: O{}",
        output.lir_facts().summary.opt_level.as_str()
    )
    .unwrap();
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
    use crate::effect_facts::CallableAbiKind;
    use crate::opt::OptLevel;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use scoopc_lir_facts::{
        LirCallSiteKind, LirCallTargetMode, LirCallableContract, LirCallableSymbolKind,
        LirGlobalRootKind, LirGlobalStoragePolicy, LirSourceCallSiteKey,
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
    } on {
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

    fn generic_interface_default_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/generic_interface_default.scoop",
            r#"
package fixtures.t5000gr

import scoop.core.*

interface Ping {
    fun ping(): Int {
        return 7
    }
}

class Box() : Ping

fun <T> use(x: T): Int where T: Ping {
    return x.ping()
}

fun main(): Int {
    return use(Box())
}
"#,
        )
    }

    #[test]
    fn effect_lowered_stage_output_is_constructible() {
        let output = run_sample();

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

        assert_eq!(
            format!("{:?}", lowered_main.call_abi_kind()),
            format!("{:?}", expected_abi)
        );
        assert_eq!(
            format!("{:?}", lowered_main.body_step_schema()),
            format!("{:?}", expected_body_step_schema)
        );
        assert_eq!(
            format!("{:?}", lowered_main.impl_plan()),
            format!("{:?}", expected_impl_plan)
        );
        assert_eq!(lowered_main.needs_reentry(), expected_needs_reentry);
        assert_eq!(
            format!("{:?}", lowered_main.resolved_outward_cases()),
            format!("{:?}", expected_cases.as_slice())
        );
        assert_eq!(
            lowered_main.plain_abi().is_some(),
            matches!(
                expected_abi,
                crate::effect_facts_stage::CallableAbiKind::Plain
            )
        );
        assert_eq!(
            output.lir_facts().summary.callable_count,
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
    fn effect_lowered_lir_facts_publish_exact_callee_binding() {
        let output = run_sample();
        let facts = output.lir_facts();
        let (main_id, main) = facts
            .callables
            .iter()
            .find(|(_, callable)| callable.root_fqn() == "sample.main")
            .expect("sample.main callable facts should be published");
        let LirCallableContract::Plain(plain) = &main.contract else {
            panic!("sample.main should publish a plain ABI contract");
        };
        let site = plain
            .call_sites
            .iter()
            .find(|site| site.contract.target_mode == LirCallTargetMode::KnownInstance)
            .expect("helper call should publish a known-instance call-site contract");
        let exact = site
            .contract
            .exact_callee
            .as_ref()
            .expect("known-instance call should publish exact callee binding");

        assert_eq!(exact.root_fqn, "sample.helper");
        assert_eq!(
            site.contract.target_callables.as_slice(),
            std::slice::from_ref(&exact.target_callable)
        );
        let signature = facts
            .source_signatures
            .get(&exact.root_fqn)
            .expect("exact callee root should resolve to a source signature");
        assert_eq!(signature.signature_key, exact.signature_key);
        let target_id = exact
            .target_callable
            .local_id()
            .expect("exact callee should resolve to a local callable id");
        let symbol = facts
            .physical_layout
            .callable_symbols
            .get(&target_id)
            .expect("exact callee should resolve to callable symbol facts");
        assert_eq!(
            symbol.exported_symbol.as_deref(),
            Some(exact.abi_symbol.as_str())
        );
        let source_site = facts
            .source_call_sites
            .get(&LirSourceCallSiteKey {
                owner_callable: *main_id,
                site_id: site.site_id,
            })
            .expect("plain call-site should also publish an identity-keyed source contract");
        assert_eq!(
            source_site
                .contract
                .exact_callee
                .as_ref()
                .map(|exact| exact.root_fqn.as_str()),
            Some("sample.helper")
        );
        assert!(facts.verify().is_ok());
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
    fn effect_lowered_stage_uses_pass_view_body_after_o2_removes_raw_site() {
        let source = generic_interface_default_source();
        let output = run_stage_with_opt_level(&source, OptLevel::O2);

        assert!(
            output
                .program()
                .callable("fixtures.t5000gr.use::<fixtures.t5000gr.Box>")
                .is_some(),
            "late lowering must consume the canonical pass-view body instead of raw MIR sites"
        );
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
        assert!(dump.contains("target=executeCase"));
        assert!(dump.contains("Call kind=Direct target_mode=KnownInstance"));
        assert!(dump.contains("callee_step=step#h"));
        assert!(dump.contains("dispatch_input_step_schema: step#h"));
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
        assert!(facts.physical_layout.layout_names.values().any(|layout| {
            layout.family == "class_vtable" && layout.layout_name == "sample.Base"
        }));
        assert!(facts.physical_layout.abi_symbols.values().any(|symbol| {
            symbol.role == "callable_export"
                && symbol.callable
                    == Some(scoopc_lir_facts::LirCallableRef::Local(
                        call_virtual.callable,
                    ))
                && Some(symbol.symbol.as_str()) == call_virtual.exported_symbol.as_deref()
        }));
        assert!(!facts.type_context.primary_fingerprint.is_empty());
        assert_eq!(
            facts.type_context.stable_wire_format.decision,
            scoopc_lir_facts::LirTypeStableWireFormatDecision::Implemented
        );
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
            "handle_continuation_binder instance=executeCase",
            "cont_obj#h",
            "site#h",
            "arm#0 handled_case=case#h",
            "source=OwnerTrampolineMixed",
            "resume_boundary instance=executeCase",
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
