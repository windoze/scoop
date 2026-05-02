use std::collections::BTreeMap;

use crate::mir::{
    File as MirFile, FunDecl as MirFunDecl, Item as MirItem, LoweredMir, MaterializedMir,
    MirLowerError, MirLoweringFacts, lower_hir_file_for_dump_with_facts,
};
use crate::ty::TypeStore;

use super::{TypedHirEffectContracts, TypedHirStageOutput};

/// refactor direct-style MIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P3/P4 及后续阶段直接消费：
/// - `lowered_mir` 仍是 direct-style MIR，而不是 late-lowered `Step` IR；
/// - 当前所有 effect-sensitive site 继续通过 MIR 节点上的 `SiteId` 锚定；
/// - `effect_contracts` 明确承载来自 P2 typed HIR stage 的 effect/continuation handoff，
///   下游不得回到 P2 的内部缓存重新拼装；
/// - `callable_body_indices` 与可选的 `materialized_mir` 把 P4 会消费的 canonical MIR
///   handoff 显式挂在 stage 输出上，而不是继续藏在 `LoweredHir` 私有字段或 dump helper 里。
#[derive(Debug)]
pub struct RefactorMirStageOutput {
    lowered_mir: LoweredMir,
    effect_contracts: TypedHirEffectContracts,
    callable_body_indices: BTreeMap<String, usize>,
    materialized_mir: Option<MaterializedMir>,
}

impl RefactorMirStageOutput {
    pub(crate) fn new(
        lowered_mir: LoweredMir,
        effect_contracts: TypedHirEffectContracts,
        materialized_mir: Option<MaterializedMir>,
    ) -> Self {
        Self {
            callable_body_indices: collect_callable_body_indices(&lowered_mir.file),
            lowered_mir,
            effect_contracts,
            materialized_mir,
        }
    }

    pub fn file(&self) -> &MirFile {
        &self.lowered_mir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_mir.types
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    /// 返回当前 stage 输出上显式挂住的 canonical materialized MIR 快照（若存在）。
    pub fn materialized_mir(&self) -> Option<&MaterializedMir> {
        self.materialized_mir.as_ref()
    }

    /// 以稳定顺序枚举当前 direct-style MIR 中可查询的 callable body 身份。
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.callable_body_indices.keys().map(String::as_str)
    }

    /// 按 callable body 身份查询 canonical direct-style MIR body。
    pub fn callable_body(&self, fqn: &str) -> Option<&MirFunDecl> {
        let item_index = *self.callable_body_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::Fun(fun) if fun.body.is_some() => Some(fun),
            _ => None,
        }
    }

    /// 当前先保持 `dump-mir` 的稳定 surface 只打印 MIR `File` Debug。
    ///
    /// refactor 专属 snapshot / golden 会在后续 P3 任务中单独冻结，不在这里提前改变 CLI 文本。
    pub fn stable_dump(&self) -> String {
        format!("{:#?}\n", self.file())
    }

    pub fn into_lowered_mir(self) -> LoweredMir {
        self.lowered_mir
    }
}

fn collect_callable_body_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::Fun(fun) = item else {
            continue;
        };
        if fun.body.is_none() {
            continue;
        }
        indices.entry(fun.fqn.clone()).or_insert(item_index);
    }
    indices
}

pub(crate) fn run(
    typed_hir_output: TypedHirStageOutput,
) -> Result<RefactorMirStageOutput, MirLowerError> {
    let effect_contracts = typed_hir_output.effect_contracts().clone();
    let mut lowered_hir = typed_hir_output.into_lowered_hir();
    let builtins = lowered_hir.types.intern_builtins();
    let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
    let materialized_mir = lowered_hir.into_materialized_mir();

    Ok(RefactorMirStageOutput::new(
        LoweredMir { file, types },
        effect_contracts,
        materialized_mir,
    ))
}

#[cfg(test)]
mod tests {
    use super::RefactorMirStageOutput;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;

    use super::super::TypedHirEffectContracts;

    #[test]
    fn refactor_direct_mir_stage_output_is_constructible() {
        let session =
            Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert_eq!(output.file().items.len(), 2);
        assert!(output.callable_body("sample.helper").is_some());
        assert!(output.callable_body("sample.main").is_some());
        assert_eq!(output.effect_contracts().function_effects().len(), 2);
        assert!(output.stable_dump().contains("FunDecl"));
    }

    #[test]
    fn refactor_direct_mir_stage_keeps_callable_body_query_surface_stable() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let output = RefactorMirStageOutput::new(
            crate::mir::LoweredMir {
                file: crate::mir::File {
                    items: vec![crate::mir::Item::Fun(crate::mir::FunDecl {
                        span: crate::span::Span::new(0, 1),
                        fqn: "sample.main".to_string(),
                        name: "main".to_string(),
                        ty: builtins.unit,
                        params: Vec::new(),
                        return_ty: builtins.unit,
                        body: Some(crate::mir::Body::new_empty()),
                    })],
                },
                types,
            },
            TypedHirEffectContracts::default(),
            None,
        );

        assert_eq!(
            output.callable_body_fqns().collect::<Vec<_>>(),
            vec!["sample.main"]
        );
        assert!(output.callable_body("sample.main").is_some());
    }
}
