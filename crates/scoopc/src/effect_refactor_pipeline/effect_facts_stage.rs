use crate::effect_facts::{
    EffectFactsError, MaterializedEffectFacts, MaterializedEffectFactsBuilder,
    MaterializedEffectFactsSolver,
};
use crate::mir::{File as MirFile, MaterializedMir, MaterializedMirPassView};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::TypeStore;

use super::RefactorMirStageOutput;

/// refactor effect-facts stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P4/P5 及后续阶段直接消费：
/// - 输入必须是 P3 的 `RefactorMirStageOutput`，而不是 AST/HIR 或 legacy effect helper；
/// - `materialized_pass_view()` 是当前 canonical MIR snapshot 的唯一查询面；P4 不混用 raw
///   `MaterializedMir.file` 与 pass-view body/summaries；
/// - `effect_facts()` 是 P5 唯一允许消费的 authoritative effect contract；P5 不得再回
///   HIR/typecheck 推断缺失语义；
/// - 一旦 MIR snapshot 发生结构性 rewrite，必须重新运行本 stage 获取新的 facts 输出。
#[derive(Debug)]
pub struct RefactorEffectFactsStageOutput {
    mir_stage_output: RefactorMirStageOutput,
    effect_facts: MaterializedEffectFacts,
}

impl RefactorEffectFactsStageOutput {
    fn new(
        mir_stage_output: RefactorMirStageOutput,
        effect_facts: MaterializedEffectFacts,
    ) -> Self {
        Self {
            mir_stage_output,
            effect_facts,
        }
    }

    pub fn mir_stage_output(&self) -> &RefactorMirStageOutput {
        &self.mir_stage_output
    }

    pub fn file(&self) -> &MirFile {
        self.mir_stage_output.file()
    }

    pub fn types(&self) -> &TypeStore {
        &self.materialized_mir().types
    }

    pub fn materialized_mir(&self) -> &MaterializedMir {
        self.mir_stage_output
            .materialized_mir()
            .expect("P4 effect-facts stage output should always retain canonical materialized MIR")
    }

    pub fn materialized_pass_view(&self) -> MaterializedMirPassView<'_> {
        self.materialized_mir().pass_view()
    }

    pub fn effect_facts(&self) -> &MaterializedEffectFacts {
        &self.effect_facts
    }

    /// refactor `dump-effect-facts` / dedicated fixtures / 定向单测共用的稳定文本 surface。
    ///
    /// 该 dump 明确锁定 P4 -> P5 handoff：只展示 canonical MIR snapshot 绑定到的
    /// `MaterializedEffectFacts`，不回 HIR/typecheck 重建缺失 effect 语义。
    pub fn stable_dump(&self) -> String {
        self.effect_facts
            .stable_dump(self.types(), self.materialized_pass_view())
    }
}

pub(crate) fn run(
    session: &Session,
    source: &SourceFile,
    mut mir_stage_output: RefactorMirStageOutput,
) -> Result<RefactorEffectFactsStageOutput, EffectFactsError> {
    let solver = MaterializedEffectFactsSolver::for_opt_level(
        mir_stage_output
            .materialized_mir()
            .ok_or(EffectFactsError::MissingMaterializedMirSnapshot)?
            .opt_level(),
    );
    let seeded_facts = {
        let materialized_mir = mir_stage_output
            .materialized_mir_mut()
            .ok_or(EffectFactsError::MissingMaterializedMirSnapshot)?;
        MaterializedEffectFactsBuilder::from_materialized_snapshot(
            session,
            source,
            materialized_mir,
        )
        .build()?
    };
    let effect_facts = solver.solve(seeded_facts);
    Ok(RefactorEffectFactsStageOutput::new(
        mir_stage_output,
        effect_facts,
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{RefactorMirStageOutput, TypedHirEffectContracts};
    use super::RefactorEffectFactsStageOutput;
    use crate::effect_facts::{CanonicalMirQuerySurface, EffectFactsError};
    use crate::mir::{File, LoweredMir};
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_stage_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    fn run_sample() -> RefactorEffectFactsStageOutput {
        let session = refactor_session();
        let source = sample_source();
        run_stage(&session, &source)
    }

    fn run_stage(session: &Session, source: &SourceFile) -> RefactorEffectFactsStageOutput {
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(session, source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        super::run(session, source, mir_stage_output)
            .expect("fixture 应可通过 refactor effect-facts stage")
    }

    fn run_stage_with_opt_level(
        source: &SourceFile,
        opt_level: crate::opt::OptLevel,
    ) -> RefactorEffectFactsStageOutput {
        let session = refactor_session();
        let materialized =
            crate::mir::materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, source)
                .unwrap()
                .with_materialized_mir(materialized);
        super::run(&session, source, mir_stage_output)
            .expect("fixture 应可通过 refactor effect-facts stage")
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
    } with {
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

    #[test]
    fn refactor_effect_facts_stage_output_is_constructible() {
        let output = run_sample();

        assert_eq!(output.file().items.len(), 2);
        assert_eq!(
            output.effect_facts().snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(
            output.effect_facts().snapshot_binding().instance_count(),
            output.materialized_pass_view().len()
        );
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.effect_facts().bodies().len()
        );
        assert!(
            output.materialized_pass_view().len() >= 2,
            "普通 non-generic sample root/helper 也应进入 canonical pass view"
        );
        assert!(
            output.effect_facts().callable_facts().contains_key(
                output
                    .materialized_pass_view()
                    .owner_of_callable("sample.main")
                    .expect("sample.main 应有 canonical InstanceKey owner")
            ),
            "effect facts stage 应直接以 canonical pass-view owner 键入普通 non-generic callable"
        );
        assert!(output.stable_dump().contains("MaterializedEffectFacts"));
    }

    #[test]
    fn refactor_effect_facts_stage_explicitly_consumes_p3_mir_stage_output() {
        let output = run_sample();

        assert!(
            output
                .mir_stage_output()
                .callable_body("sample.main")
                .is_some()
        );
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.materialized_pass_view().len()
        );
        assert_eq!(
            output.effect_facts().bodies().len(),
            output.materialized_pass_view().len()
        );
    }

    #[test]
    fn refactor_effect_facts_stage_requires_materialized_snapshot() {
        let output = RefactorMirStageOutput::new(
            LoweredMir {
                file: File { items: Vec::new() },
                types: TypeStore::new(),
            },
            TypedHirEffectContracts::default(),
            None,
        );

        let session = refactor_session();
        let source = sample_source();
        let err = super::run(&session, &source, output).unwrap_err();

        assert!(matches!(
            err,
            EffectFactsError::MissingMaterializedMirSnapshot
        ));
    }

    #[test]
    fn refactor_effect_facts_stage_non_generic_sample_main_uses_canonical_pass_view_roots() {
        let output = run_sample();
        let main_key = output
            .materialized_pass_view()
            .owner_of_callable("sample.main")
            .expect("sample.main 应被 canonical pass view 发布")
            .clone();

        assert!(
            output
                .effect_facts()
                .callable_facts()
                .contains_key(&main_key),
            "effect facts stage 应以 pass-view canonical InstanceKey 发布 ordinary callable facts"
        );
        assert!(
            output.effect_facts().bodies().contains_key(&main_key),
            "effect facts stage 应以同一 canonical InstanceKey 发布 ordinary body facts"
        );
    }

    #[test]
    fn refactor_effect_facts_stage_non_generic_sample_helper_uses_canonical_pass_view_roots() {
        let output = run_sample();
        let helper_key = output
            .materialized_pass_view()
            .owner_of_callable("sample.helper")
            .expect("sample.helper 应被 canonical pass view 发布")
            .clone();

        assert!(
            output
                .effect_facts()
                .callable_facts()
                .contains_key(&helper_key),
            "ordinary helper facts 不应再依赖 raw/fallback 键空间"
        );
        assert!(
            output.effect_facts().bodies().contains_key(&helper_key),
            "ordinary helper body facts 应可直接按 canonical InstanceKey 查询"
        );
    }

    #[test]
    fn refactor_effect_facts_stage_stable_dump_lists_schema_callable_and_site_sections() {
        let session = refactor_session();
        let source = dump_fixture_source();
        let output = run_stage(&session, &source);
        let dump = output.stable_dump();

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
    }

    #[test]
    fn refactor_effect_facts_stage_stable_dump_locks_opt_level_visible_impl_plan() {
        let source = single_case_source();
        let o0 = run_stage_with_opt_level(&source, crate::opt::OptLevel::O0).stable_dump();
        let o2 = run_stage_with_opt_level(&source, crate::opt::OptLevel::O2).stable_dump();

        assert!(o0.contains("opt_level: O0"));
        assert!(o0.contains("impl_plan: CanonicalFull"));
        assert!(o2.contains("opt_level: O2"));
        assert!(o2.contains("impl_plan: SingleCase(case#0=sample.Ping.hit)"));
    }

    #[test]
    fn refactor_effect_facts_stage_supports_declaration_only_interface_surface_contracts() {
        let session = refactor_session();
        let source = declaration_only_interface_source();
        let dump = run_stage(&session, &source).stable_dump();

        assert!(dump.contains("sample.callInterface:"));
        assert!(dump.contains("target: KnownInstance(sample.IFace.foo)"));
        assert!(dump.contains("callee_schema: step_schema#"));
    }
}
