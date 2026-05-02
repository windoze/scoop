use crate::mir::MirLowerError;
use crate::parser::ParseError;
use crate::session::EffectPipelineMode;
use crate::session::Session;
use crate::source::SourceFile;

use super::{
    AstStageOutput, RefactorEffectFactsStageOutput, RefactorMirStageOutput, StageKind,
    TypedHirStageOutput, ast_stage, effect_facts_stage, hir_stage, mir_stage,
};

/// refactor 主线的阶段入口。
///
/// P0 先固定入口形状；P1 / P2 已分别填入 AST / typed HIR stage，剩余阶段后续继续逐步替换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageEntry {
    stage: StageKind,
}

impl StageEntry {
    pub(crate) const fn new(stage: StageKind) -> Self {
        Self { stage }
    }

    pub(crate) const fn mode(self) -> EffectPipelineMode {
        EffectPipelineMode::Refactor
    }

    pub(crate) const fn stage(self) -> StageKind {
        self.stage
    }

    pub(crate) fn parse_ast_stage_output<'a>(
        self,
        session: &Session,
        source: &'a SourceFile,
    ) -> Result<AstStageOutput<'a>, ParseError> {
        debug_assert_eq!(self.stage, StageKind::Ast);
        let _ = self;
        ast_stage::run(session, source)
    }

    pub(crate) fn lower_typed_hir_stage_output(
        self,
        session: &Session,
        source: &SourceFile,
    ) -> Result<TypedHirStageOutput, crate::hir::HirLowerError> {
        debug_assert_eq!(self.stage, StageKind::TypedHir);
        let _ = self;
        hir_stage::run(session, source)
    }

    pub(crate) fn lower_direct_style_mir_stage_output(
        self,
        typed_hir_output: TypedHirStageOutput,
    ) -> Result<RefactorMirStageOutput, MirLowerError> {
        debug_assert_eq!(self.stage, StageKind::DirectStyleMir);
        let _ = self;
        mir_stage::run(typed_hir_output)
    }

    pub(crate) fn lower_effect_facts_stage_output(
        self,
        mir_stage_output: RefactorMirStageOutput,
    ) -> Result<RefactorEffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
        debug_assert_eq!(self.stage, StageKind::EffectFacts);
        let _ = self;
        effect_facts_stage::run(mir_stage_output)
    }

    pub(crate) fn delegate_to_legacy<T>(self, legacy_op: impl FnOnce() -> T) -> T {
        let _ = self;
        legacy_op()
    }
}
