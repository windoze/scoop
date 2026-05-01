use crate::hir::{HirLowerError, LoweredHir};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::TypeStore;

/// P2 typed HIR stage 产物上预留的 effect / continuation contract side-table 容器。
///
/// P2-T01 先把它固定为稳定挂点；具体结构化 contract 会在后续 P2-T03 / P2-T04 中补齐。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TypedHirEffectContracts;

impl TypedHirEffectContracts {
    pub const fn is_placeholder(&self) -> bool {
        true
    }
}

/// refactor typed HIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P2/P3 及后续阶段直接消费：
/// - 输出已经过 resolver + typecheck，可直接视为 typed HIR handoff；
/// - `Continuation` / `resume` / `perform` / `handle` 的 typed contract 应在此阶段显式化，
///   下游不应再回 AST 猜测 surface 语义；
/// - `dump-hir` 的 refactor 路径必须优先消费这一 stage 输出，而不是 legacy
///   `hir::lower_for_dump(...)`；
/// - effect / continuation side tables 在 P2-T01 先保留稳定挂点，后续任务再补齐结构化内容。
#[derive(Debug)]
pub struct TypedHirStageOutput {
    lowered_hir: LoweredHir,
    effect_contracts: TypedHirEffectContracts,
}

impl TypedHirStageOutput {
    pub(crate) fn new(lowered_hir: LoweredHir) -> Self {
        Self {
            lowered_hir,
            effect_contracts: TypedHirEffectContracts,
        }
    }

    pub fn hir_file(&self) -> &crate::hir::File {
        &self.lowered_hir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_hir.types
    }

    pub fn lowered_hir(&self) -> &LoweredHir {
        &self.lowered_hir
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    pub fn into_lowered_hir(self) -> LoweredHir {
        self.lowered_hir
    }
}

pub(crate) fn run(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, HirLowerError> {
    let lowered_hir = crate::hir::lower_typed_for_dump(session, source)?;
    Ok(TypedHirStageOutput::new(lowered_hir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{EffectPipelineMode, SessionOptions};

    #[test]
    fn refactor_typed_hir_stage_output_is_constructible() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert_eq!(output.hir_file().items.len(), 1);
        assert!(output.effect_contracts().is_placeholder());
    }

    #[test]
    fn refactor_typed_hir_stage_keeps_placeholder_contract_shell() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(!output.types().is_empty());
        assert!(output.effect_contracts().is_placeholder());
    }
}
