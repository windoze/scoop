use crate::effect_facts_stage::{
    BodyEffectFacts, EffectFactsError, EffectOwnedTypeContext, MaterializedEffectFacts,
    MaterializedEffectFactsBuilder, MaterializedEffectFactsSolver, SiteEffectFacts,
};

use super::MirStageOutput;

/// effect-facts stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P4/P5 及后续阶段直接消费：
/// - 输入必须是 P3 的 `MirStageOutput`，而不是 AST/HIR 或 legacy effect helper；
/// - `effect_facts()` 是 P5 唯一允许消费的 authoritative effect contract；P5 不得再回
///   HIR/typecheck 推断缺失语义；
/// - 输出只发布 effect facts 及其 effect-owned context/snapshot binding，不嵌套或转发 P3
///   `MirStageOutput`。
#[derive(Debug)]
pub struct EffectFactsStageOutput {
    effect_facts: MaterializedEffectFacts,
    published_effect_facts: scoopc_effect_facts::EffectFacts,
}

impl EffectFactsStageOutput {
    fn new(
        effect_facts: MaterializedEffectFacts,
        published_effect_facts: scoopc_effect_facts::EffectFacts,
    ) -> Self {
        Self {
            effect_facts,
            published_effect_facts,
        }
    }

    pub fn effect_facts(&self) -> &MaterializedEffectFacts {
        &self.effect_facts
    }

    pub fn published_effect_facts(&self) -> &scoopc_effect_facts::EffectFacts {
        &self.published_effect_facts
    }

    pub fn effect_types(&self) -> &crate::ty::TypeStore {
        self.effect_facts.types()
    }

    #[cfg_attr(not(feature = "llvm"), allow(dead_code))]
    pub(crate) fn into_effect_facts(self) -> MaterializedEffectFacts {
        self.effect_facts
    }

    /// `dump-effect-facts` / dedicated fixtures / 定向单测共用的稳定文本 surface。
    ///
    /// 该 dump 明确锁定 P4 output 的窄 handoff：只展示 effect facts、effect-owned type
    /// context 与 snapshot binding，不通过 P4 output 回看 MIR pass view。
    pub fn stable_dump(&self) -> String {
        self.effect_facts.stable_dump()
    }
}

pub(crate) fn run(
    mir_stage_output: &MirStageOutput,
) -> Result<EffectFactsStageOutput, EffectFactsError> {
    let frontend_artifact =
        mir_stage_output
            .hir_semantic_artifact()
            .ok_or_else(|| EffectFactsError::Frontend {
                message: "P4 effect-facts stage 缺少 HIR semantic artifact".to_string(),
            })?;
    let solver = MaterializedEffectFactsSolver::for_opt_level(
        mir_stage_output.materialized_mir().opt_level(),
    );
    let mut type_context =
        EffectOwnedTypeContext::from_mir_types(&mir_stage_output.materialized_mir().types);
    let seeded_facts = {
        MaterializedEffectFactsBuilder::from_materialized_snapshot(
            frontend_artifact,
            mir_stage_output.materialized_mir(),
            mir_stage_output.mir_facts(),
            &mut type_context,
        )
        .build()?
    };
    let provisional_facts = solver.solve(seeded_facts);
    let compiler_continuation_runtime_error_callables = provisional_facts
        .callable_facts()
        .iter()
        .filter_map(|(key, callable)| {
            (callable.needs_reentry()
                || provisional_facts
                    .body(key)
                    .is_some_and(body_has_escaped_continuation))
            .then_some(key.clone())
        })
        .collect::<Vec<_>>();
    let effect_facts = if compiler_continuation_runtime_error_callables.is_empty() {
        provisional_facts
    } else {
        // 第二次构建把 compiler-generated continuation 的 one-shot runtime error 正式纳入
        // step-schema 上界；solver 仍只会在真实 body/site outward 贡献存在时把它留进
        // resolved_outward_cases。
        let seeded_facts = {
            MaterializedEffectFactsBuilder::from_materialized_snapshot(
                frontend_artifact,
                mir_stage_output.materialized_mir(),
                mir_stage_output.mir_facts(),
                &mut type_context,
            )
            .with_compiler_continuation_runtime_error_callables(
                compiler_continuation_runtime_error_callables,
            )
            .build()?
        };
        solver.solve(seeded_facts)
    };
    let published_effect_facts =
        effect_facts.to_published_effect_facts(mir_stage_output.materialized_pass_view())?;
    Ok(EffectFactsStageOutput::new(
        effect_facts,
        published_effect_facts,
    ))
}

fn body_has_escaped_continuation(body: &BodyEffectFacts) -> bool {
    body.sites().values().any(|site| {
        matches!(
            site,
            SiteEffectFacts::Handle(handle) if !handle.arm_facts().is_empty()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::EffectFactsStageOutput;
    use crate::effect_facts_stage::{
        CallableAbiKind, CanonicalMirQuerySurface, ImplPlan, MirSnapshotBinding, SiteEffectFacts,
    };
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_stage_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    fn run_sample() -> EffectFactsStageOutput {
        let session = session();
        let source = sample_source();
        run_stage(&session, &source)
    }

    fn run_stage(session: &Session, source: &SourceFile) -> EffectFactsStageOutput {
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(session, source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        super::run(&mir_stage_output).expect("fixture 应可通过 effect-facts stage")
    }

    fn run_stage_with_opt_level(
        source: &SourceFile,
        opt_level: crate::opt::OptLevel,
    ) -> EffectFactsStageOutput {
        let session = session();
        let materialized =
            crate::mir::materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        super::run(&mir_stage_output).expect("fixture 应可通过 effect-facts stage")
    }

    fn type_store_fingerprint(types: &crate::ty::TypeStore) -> Vec<String> {
        types
            .iter_ids()
            .map(|id| format!("{id:?}:{:?}", types.kind(id)))
            .collect()
    }

    fn dump_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_dump_fixture.scoop",
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
            "<mem>/effect_facts_single_case_fixture.scoop",
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

    fn compiler_continuation_runtime_error_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_compiler_cont_runtime_error_stage_fixture.scoop",
            r#"
package sample

effect Ping {
    fun hit(): Unit
}

fun leaf(): Unit / Ping {
    Ping.hit()
}

fun pureHelper(): Unit {}
"#,
        )
    }

    fn declaration_only_interface_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_interface_surface_fixture.scoop",
            r#"
package sample

interface IFace {
    fun foo(): Int
}

fun callInterface(i: IFace): Int {
    return i.foo()
}
"#,
        )
    }

    fn schema_case_fqns(
        output: &EffectFactsStageOutput,
        step_schema: crate::effect_facts_stage::StepSchemaId,
    ) -> BTreeSet<String> {
        output
            .effect_facts()
            .step_schemas()
            .get(&step_schema)
            .expect("step schema 应存在")
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect()
    }

    fn case_set_fqns(
        output: &EffectFactsStageOutput,
        case_set: &crate::effect_facts_stage::CaseSet,
    ) -> BTreeSet<String> {
        if case_set.is_empty() {
            return BTreeSet::new();
        }
        let schema = output
            .effect_facts()
            .step_schemas()
            .get(&case_set.schema())
            .expect("case set 应引用已存在的 step schema");
        case_set
            .tags()
            .iter()
            .map(|tag| {
                schema
                    .cases()
                    .iter()
                    .find(|case| case.case_tag() == *tag)
                    .expect("case tag 应落在对应 schema 中")
                    .concrete_op_key()
                    .instance_key()
                    .template
                    .fqn
                    .clone()
            })
            .collect()
    }

    fn continuation_surface_tys_for_step_schema(
        output: &EffectFactsStageOutput,
        step_schema: crate::effect_facts_stage::StepSchemaId,
    ) -> BTreeSet<String> {
        output
            .effect_facts()
            .step_schemas()
            .get(&step_schema)
            .expect("step schema 应存在")
            .cases()
            .iter()
            .map(|case| {
                let schema = output
                    .effect_facts()
                    .continuation_schemas()
                    .get(&case.continuation_schema())
                    .expect("continuation schema 应存在");
                output
                    .effect_facts()
                    .types()
                    .display(schema.surface_ty())
                    .to_string()
            })
            .collect()
    }

    fn callable_facts_for<'a>(
        output: &'a EffectFactsStageOutput,
        fqn: &str,
    ) -> &'a crate::effect_facts_stage::CallableEffectFacts {
        output
            .effect_facts()
            .callable_facts()
            .iter()
            .find_map(|(key, facts)| (key.template.fqn == fqn).then_some(facts))
            .unwrap_or_else(|| panic!("{fqn} 应存在于 callable facts"))
    }

    #[test]
    fn effect_facts_stage_output_is_constructible() {
        let session = session();
        let source = sample_source();
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(&session, &source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .with_materialized_mir(materialized);
        let output = super::run(&mir_stage_output).expect("fixture 应可通过 effect-facts stage");

        assert_eq!(mir_stage_output.file().items.len(), 2);
        assert_eq!(
            output.effect_facts().snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(
            output.effect_facts().snapshot_binding().instance_count(),
            mir_stage_output.materialized_pass_view().len()
        );
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.effect_facts().bodies().len()
        );
        assert!(
            mir_stage_output.materialized_pass_view().len() >= 2,
            "普通 non-generic sample root/helper 也应进入 canonical pass view"
        );
        assert!(
            output.effect_facts().callable_facts().contains_key(
                mir_stage_output
                    .materialized_pass_view()
                    .owner_of_callable("sample.main")
                    .expect("sample.main 应有 canonical InstanceKey owner")
            ),
            "effect facts stage 应直接以 canonical pass-view owner 键入普通 non-generic callable"
        );
        let published = output
            .effect_facts()
            .to_published_effect_facts(mir_stage_output.materialized_pass_view())
            .expect("materialized facts 应可适配为独立 scoopc_effect_facts 产品");
        assert_eq!(
            published.callables.len(),
            output.effect_facts().callable_facts().len()
        );
        assert!(published.verify().is_ok());
        assert!(output.stable_dump().contains("MaterializedEffectFacts"));
    }

    #[test]
    fn effect_facts_stage_explicitly_consumes_p3_mir_stage_output() {
        let session = session();
        let source = sample_source();
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(&session, &source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .with_materialized_mir(materialized);
        let output = super::run(&mir_stage_output).expect("fixture 应可通过 effect-facts stage");

        assert!(mir_stage_output.callable_body("sample.main").is_some());
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            mir_stage_output.materialized_pass_view().len()
        );
        assert_eq!(
            output.effect_facts().bodies().len(),
            mir_stage_output.materialized_pass_view().len()
        );
        let mir_facts = mir_stage_output.mir_facts();
        assert!(
            mir_facts.snapshots.canonical.is_some(),
            "P4-ready MIR output must publish a canonical snapshot binding"
        );
        assert_eq!(
            mir_facts.families.instances.len(),
            mir_stage_output.materialized_pass_view().len(),
            "MIR facts should publish the pass-visible instance inventory"
        );
        let pass_names = mir_facts
            .pass_pipeline
            .runs
            .iter()
            .map(|run| run.pass.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            pass_names,
            vec![
                "devirtualization",
                "summary-driven-inlining",
                "escape-analysis",
                "closure-simplification"
            ]
        );
        assert_eq!(mir_facts.pass_artifacts.revisions.len(), 5);
        assert_eq!(
            mir_facts.pass_artifacts.summary_revisions.len(),
            mir_stage_output.materialized_pass_view().len(),
            "canonical pass summaries should be visible as MIR-owned artifacts"
        );
    }

    #[test]
    fn effect_facts_stage_does_not_mutate_mir_handoff() {
        let session = session();
        let source = compiler_continuation_runtime_error_source();
        let materialized = crate::mir::materialize_for_dump_with_opt_level(
            &session,
            &source,
            crate::opt::OptLevel::O2,
        )
        .unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .with_materialized_mir(materialized);
        let before_binding =
            MirSnapshotBinding::from_pass_view(&mir_stage_output.materialized_pass_view());
        let before_pass_artifacts =
            format!("{:?}", mir_stage_output.materialized_mir().pass_artifacts());
        let before_types = type_store_fingerprint(&mir_stage_output.materialized_mir().types);

        let output =
            super::run(&mir_stage_output).expect("fixture 应可通过只读 effect-facts stage");

        assert_eq!(
            MirSnapshotBinding::from_pass_view(&mir_stage_output.materialized_pass_view()),
            before_binding,
            "effect facts stage 不应改写 canonical snapshot binding"
        );
        assert_eq!(
            format!("{:?}", mir_stage_output.materialized_mir().pass_artifacts()),
            before_pass_artifacts,
            "effect facts stage 不应改写 MIR pass artifacts metadata"
        );
        assert_eq!(
            type_store_fingerprint(&mir_stage_output.materialized_mir().types),
            before_types,
            "P4-owned type additions 不应写回 MIR TypeStore"
        );
        assert!(
            output.effect_facts().types().len() >= mir_stage_output.materialized_mir().types.len(),
            "effect facts output 应发布可覆盖 MIR types 的 effect-owned type context"
        );
    }

    #[test]
    fn effect_facts_stage_non_generic_sample_main_uses_canonical_pass_view_roots() {
        let output = run_sample();
        let (main_key, _) = output
            .effect_facts()
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == "sample.main")
            .expect("sample.main 应被 effect facts 发布");

        assert!(
            output
                .effect_facts()
                .callable_facts()
                .contains_key(main_key),
            "effect facts stage 应以 pass-view canonical InstanceKey 发布 ordinary callable facts"
        );
        assert!(
            output.effect_facts().bodies().contains_key(main_key),
            "effect facts stage 应以同一 canonical InstanceKey 发布 ordinary body facts"
        );
    }

    #[test]
    fn effect_facts_stage_non_generic_sample_helper_uses_canonical_pass_view_roots() {
        let output = run_sample();
        let (helper_key, _) = output
            .effect_facts()
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == "sample.helper")
            .expect("sample.helper 应被 effect facts 发布");

        assert!(
            output
                .effect_facts()
                .callable_facts()
                .contains_key(helper_key),
            "ordinary helper facts 不应再依赖 raw/fallback 键空间"
        );
        assert!(
            output.effect_facts().bodies().contains_key(helper_key),
            "ordinary helper body facts 应可直接按 canonical InstanceKey 查询"
        );
    }

    #[test]
    fn effect_facts_stage_stable_dump_lists_schema_callable_and_site_sections() {
        let session = session();
        let source = dump_fixture_source();
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(&session, &source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .with_materialized_mir(materialized);
        let output = super::run(&mir_stage_output).expect("fixture 应可通过 effect-facts stage");
        let dump = output.stable_dump();
        let published = output
            .effect_facts()
            .to_published_effect_facts(mir_stage_output.materialized_pass_view())
            .expect("effect/control schema graph 应可适配为独立 fact 产品");

        assert!(dump.contains("snapshot_binding:"));
        assert!(dump.contains("step_schemas:"));
        assert!(dump.contains("continuation_schemas:"));
        assert!(dump.contains("callable_facts:"));
        assert!(dump.contains("body_facts:"));
        assert!(dump.contains("kind: Perform"));
        assert!(dump.contains("kind: Resume"));
        assert!(dump.contains("kind: Handle"));
        assert!(dump.contains("impl_plan:"));
        assert!(dump.contains("nested_handle_classification:"));
        assert!(!published.step_schemas.is_empty());
        assert!(!published.continuation_schemas.is_empty());
    }

    #[test]
    fn effect_facts_stage_publishes_plain_local_control_owner_schema() {
        let output = run_stage(&session(), &dump_fixture_source());
        let (handled_key, handled_facts) = output
            .effect_facts()
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == "sample.handled")
            .expect("sample.handled 应发布 callable facts");
        let handled_body = output
            .effect_facts()
            .body(handled_key)
            .expect("sample.handled 应发布 body facts");

        assert_eq!(handled_facts.call_abi_kind(), CallableAbiKind::Plain);
        assert!(handled_facts.body_step_schema().is_none());
        assert!(
            handled_body
                .sites()
                .values()
                .any(|site| matches!(site, SiteEffectFacts::Handle(_))),
            "fixture 应包含 self-contained handle，触发 plain local control"
        );
        let local_control_step = handled_body
            .local_control_step_schema()
            .expect("plain local-control body 必须由 P4 发布 owner StepSchema");
        assert!(
            output
                .effect_facts()
                .step_schemas()
                .contains_key(&local_control_step),
            "local_control_step_schema 必须引用 P4 发布的 StepSchema"
        );
    }

    #[test]
    fn effect_facts_stage_stable_dump_locks_opt_level_visible_impl_plan() {
        let source = single_case_source();
        let o0 = run_stage_with_opt_level(&source, crate::opt::OptLevel::O0).stable_dump();
        let o2 = run_stage_with_opt_level(&source, crate::opt::OptLevel::O2).stable_dump();

        assert!(o0.contains("opt_level: O0"));
        assert!(o0.contains("impl_plan: CanonicalFull"));
        assert!(o2.contains("opt_level: O2"));
        assert!(o2.contains("impl_plan: SingleCase("));
        assert!(o2.contains("sample.Ping.hit"));
    }

    #[test]
    fn effect_facts_stage_compiler_continuation_runtime_error_keeps_runtime_error_in_schema_upper_bound()
     {
        let source = compiler_continuation_runtime_error_source();
        let output = run_stage_with_opt_level(&source, crate::opt::OptLevel::O2);
        let leaf_facts = callable_facts_for(&output, "sample.leaf");
        let pure_facts = callable_facts_for(&output, "sample.pureHelper");

        assert_eq!(
            schema_case_fqns(&output, leaf_facts.step_schema()),
            [
                "sample.Ping.hit".to_string(),
                "scoop.core.Raise.raise".to_string(),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            case_set_fqns(&output, leaf_facts.resolved_outward_cases()),
            ["sample.Ping.hit".to_string()].into_iter().collect(),
            "compiler-generated continuation runtime error 当前应只进入 step-schema 上界，不应无端扩大 leaf 的 resolved_outward_cases"
        );
        assert!(matches!(leaf_facts.impl_plan(), ImplPlan::SingleCase(_)));

        assert!(
            pure_facts.body_step_schema().is_none(),
            "truly no-outward callable 不应发布 body step schema"
        );
        assert!(pure_facts.resolved_outward_cases().is_empty());
        assert!(matches!(pure_facts.impl_plan(), ImplPlan::NoOutward));
    }

    #[test]
    fn effect_facts_stage_surface_ty_distinguishes_step_upper_bound_for_compiler_runtime_error() {
        let source = compiler_continuation_runtime_error_source();
        let output = run_stage_with_opt_level(&source, crate::opt::OptLevel::O2);
        let leaf_facts = callable_facts_for(&output, "sample.leaf");

        assert_eq!(
            continuation_surface_tys_for_step_schema(&output, leaf_facts.step_schema()),
            [
                "scoop.core.Continuation<Nothing, Unit, eff sample.Ping>".to_string(),
                "scoop.core.Continuation<Unit, Unit, eff sample.Ping>".to_string(),
            ]
            .into_iter()
            .collect(),
            "P4 authoritative handoff 必须允许 step upper bound 含 compiler-generated runtime-error case，同时保持 continuation surface_ty 只表达源码 residual row"
        );
    }

    #[test]
    fn effect_facts_stage_supports_declaration_only_interface_surface_contracts() {
        let session = session();
        let source = declaration_only_interface_source();
        let dump = run_stage(&session, &source).stable_dump();

        assert!(dump.contains("sample.callInterface:"));
        assert!(dump.contains("callee_abi_kind: Plain"));
        assert!(dump.contains("callee_schema: <none>"));
    }
}
