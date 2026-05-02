use crate::effect_facts::{
    EffectFactsError, MaterializedEffectFacts, MaterializedEffectFactsBuilder,
    MaterializedEffectFactsSolver,
};
use crate::mir::{File as MirFile, MaterializedMir, MaterializedMirPassView};
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
        self.mir_stage_output.types()
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

    pub fn stable_dump(&self) -> String {
        self.effect_facts.stable_dump()
    }
}

pub(crate) fn run(
    mir_stage_output: RefactorMirStageOutput,
) -> Result<RefactorEffectFactsStageOutput, EffectFactsError> {
    let seeded_facts = {
        let materialized_mir = mir_stage_output
            .materialized_mir()
            .ok_or(EffectFactsError::MissingMaterializedMirSnapshot)?;
        MaterializedEffectFactsBuilder::from_materialized_snapshot(materialized_mir).build()
    };
    let effect_facts = MaterializedEffectFactsSolver.solve(seeded_facts);
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
        let materialized =
            super::super::materialize_direct_style_mir_for_dump(&session, &source).unwrap();
        let mir_stage_output =
            super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
                .unwrap()
                .with_materialized_mir(materialized);
        super::run(mir_stage_output).expect("fixture 应可通过 refactor effect-facts stage")
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

        let err = super::run(output).unwrap_err();

        assert!(matches!(
            err,
            EffectFactsError::MissingMaterializedMirSnapshot
        ));
    }
}
