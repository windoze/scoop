//! effect-refactor 并行主线的顶层 dispatcher 壳层。
//!
//! P0 约束：
//! - 新旧主线只允许在这里分流；
//! - `refactor` 路径当前仍可在阶段边界整体委托给 legacy 实现；
//! - 低层业务模块不应自行读取 pipeline mode。

mod ast_stage;
mod effect_facts_stage;
mod effect_lowering_stage;
mod hir_stage;
mod legacy;
#[cfg(feature = "llvm")]
mod llvm_codegen_stage;
mod mir_stage;
mod refactor;

use crate::session::{EffectPipelineMode, Session};
use crate::source::SourceFile;

pub use ast_stage::AstStageOutput;
pub use effect_facts_stage::RefactorEffectFactsStageOutput;
pub use effect_lowering_stage::RefactorEffectLoweredStageOutput;
pub use hir_stage::{
    ContinuationResumeSiteContract, FunctionEffectContract, HandleArmContractKind,
    HandleArmSiteContract, HandleSiteContract, PayloadTypeContract, PerformSiteContract,
    TypedCallSiteKind, TypedHirEffectContracts, TypedHirStageOutput,
};
#[cfg(feature = "llvm")]
pub use llvm_codegen_stage::{RefactorLlvmCodegenStageInput, RefactorLlvmCodegenStageOutput};
pub use mir_stage::RefactorMirStageOutput;

#[cfg(feature = "llvm")]
use crate::source::{SourceId, SourceMap};
#[cfg(feature = "llvm")]
use std::path::Path;

/// P0 预留的阶段边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    Ast,
    TypedHir,
    DirectStyleMir,
    EffectFacts,
    LateLowering,
    LlvmCodegen,
}

/// 基于 session 中的 pipeline mode 选择阶段入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineDispatcher {
    mode: EffectPipelineMode,
}

impl PipelineDispatcher {
    pub fn for_session(session: &Session) -> Self {
        Self {
            mode: session.effect_pipeline_mode(),
        }
    }

    pub const fn mode(self) -> EffectPipelineMode {
        self.mode
    }

    pub fn ast(self) -> StageDispatcher {
        self.stage(StageKind::Ast)
    }

    pub fn typed_hir(self) -> StageDispatcher {
        self.stage(StageKind::TypedHir)
    }

    pub fn direct_style_mir(self) -> StageDispatcher {
        self.stage(StageKind::DirectStyleMir)
    }

    pub fn effect_facts(self) -> StageDispatcher {
        self.stage(StageKind::EffectFacts)
    }

    pub fn late_lowering(self) -> StageDispatcher {
        self.stage(StageKind::LateLowering)
    }

    pub fn llvm_codegen(self) -> StageDispatcher {
        self.stage(StageKind::LlvmCodegen)
    }

    fn stage(self, stage: StageKind) -> StageDispatcher {
        let entry = match self.mode {
            EffectPipelineMode::Legacy => StageEntry::Legacy(legacy::StageEntry::new(stage)),
            EffectPipelineMode::Refactor => StageEntry::Refactor(refactor::StageEntry::new(stage)),
        };
        StageDispatcher { entry }
    }
}

/// 单个阶段的已选入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDispatcher {
    entry: StageEntry,
}

impl StageDispatcher {
    pub const fn mode(self) -> EffectPipelineMode {
        match self.entry {
            StageEntry::Legacy(entry) => entry.mode(),
            StageEntry::Refactor(entry) => entry.mode(),
        }
    }

    pub const fn stage(self) -> StageKind {
        match self.entry {
            StageEntry::Legacy(entry) => entry.stage(),
            StageEntry::Refactor(entry) => entry.stage(),
        }
    }

    /// 尚未拥有独立实现的阶段，可在阶段边界整体委托给 legacy 实现。
    pub fn delegate_to_legacy<T>(self, legacy_op: impl FnOnce() -> T) -> T {
        match self.entry {
            StageEntry::Legacy(entry) => entry.delegate_to_legacy(legacy_op),
            StageEntry::Refactor(entry) => entry.delegate_to_legacy(legacy_op),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageEntry {
    Legacy(legacy::StageEntry),
    Refactor(refactor::StageEntry),
}

pub fn dispatcher_for_session(session: &Session) -> PipelineDispatcher {
    PipelineDispatcher::for_session(session)
}

pub fn enter_ast_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .ast()
        .delegate_to_legacy(legacy_op)
}

pub fn enter_typed_hir_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .typed_hir()
        .delegate_to_legacy(legacy_op)
}

pub fn enter_direct_style_mir_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .direct_style_mir()
        .delegate_to_legacy(legacy_op)
}

pub fn enter_effect_facts_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .effect_facts()
        .delegate_to_legacy(legacy_op)
}

pub fn enter_late_lowering_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .late_lowering()
        .delegate_to_legacy(legacy_op)
}

pub fn enter_llvm_codegen_stage<T>(session: &Session, legacy_op: impl FnOnce() -> T) -> T {
    dispatcher_for_session(session)
        .llvm_codegen()
        .delegate_to_legacy(legacy_op)
}

pub fn parse_ast_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::ast::File, crate::parser::ParseError> {
    load_ast_stage_output_for_dump(session, source).map(AstStageOutput::into_ast)
}

pub fn load_ast_stage_output_for_dump<'a>(
    session: &Session,
    source: &'a SourceFile,
) -> Result<AstStageOutput<'a>, crate::parser::ParseError> {
    let dispatcher = dispatcher_for_session(session).ast();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => entry.delegate_to_legacy(|| {
            session
                .parse(source)
                .map(|ast| AstStageOutput::new(source, ast))
        }),
        StageEntry::Refactor(entry) => entry.parse_ast_stage_output(session, source),
    }
}

pub fn load_typed_hir_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, crate::hir::HirLowerError> {
    let dispatcher = dispatcher_for_session(session).typed_hir();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => entry.delegate_to_legacy(|| {
            crate::hir::lower_typed_for_dump(session, source)
                .map(|lowered| TypedHirStageOutput::new(lowered, source.path()))
        }),
        StageEntry::Refactor(entry) => entry.lower_typed_hir_stage_output(session, source),
    }
}

pub fn lower_typed_hir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::hir::LoweredHir, crate::hir::HirLowerError> {
    let dispatcher = dispatcher_for_session(session).typed_hir();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => {
            entry.delegate_to_legacy(|| crate::hir::lower_for_dump(session, source))
        }
        StageEntry::Refactor(entry) => entry
            .lower_typed_hir_stage_output(session, source)
            .map(TypedHirStageOutput::into_lowered_hir),
    }
}

pub fn lower_direct_style_mir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::mir::LoweredMir, crate::mir::MirLowerError> {
    match session.effect_pipeline_mode() {
        EffectPipelineMode::Legacy => {
            enter_direct_style_mir_stage(session, || crate::mir::lower_for_dump(session, source))
        }
        EffectPipelineMode::Refactor => {
            load_direct_style_mir_stage_output_for_dump(session, source)
                .map(RefactorMirStageOutput::into_lowered_mir)
        }
    }
}

pub fn load_direct_style_mir_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<RefactorMirStageOutput, crate::mir::MirLowerError> {
    let typed_hir_output = load_typed_hir_stage_output_for_dump(session, source)
        .map_err(crate::mir::MirLowerError::from)?;
    let dispatcher = dispatcher_for_session(session).direct_style_mir();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => entry.delegate_to_legacy(|| mir_stage::run(typed_hir_output)),
        StageEntry::Refactor(entry) => entry.lower_direct_style_mir_stage_output(typed_hir_output),
    }
}

pub fn build_effect_facts_stage_output(
    session: &Session,
    source: &SourceFile,
    mir_stage_output: RefactorMirStageOutput,
) -> Result<RefactorEffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
    // P4 facts 必须绑定到 canonical materialized MIR snapshot。
    // 当前 P3 dump stage 仍允许在未保留 snapshot 的情况下独立产出 direct-style MIR，
    // 因此在 effect-facts stage 边界用同一 session/source 路由补挂 canonical snapshot。
    let mir_stage_output = if mir_stage_output.materialized_mir().is_some() {
        mir_stage_output
    } else {
        let materialized = materialize_direct_style_mir_for_dump(session, source)?;
        mir_stage_output.with_materialized_mir(materialized)
    };
    let dispatcher = dispatcher_for_session(session).effect_facts();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => {
            entry.delegate_to_legacy(|| effect_facts_stage::run(session, source, mir_stage_output))
        }
        StageEntry::Refactor(entry) => {
            entry.lower_effect_facts_stage_output(session, source, mir_stage_output)
        }
    }
}

pub fn load_effect_facts_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<RefactorEffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
    let mir_stage_output = load_direct_style_mir_stage_output_for_dump(session, source)
        .map_err(crate::effect_facts::EffectFactsError::from)?;
    build_effect_facts_stage_output(session, source, mir_stage_output)
}

pub fn build_effect_lowered_stage_output(
    session: &Session,
    effect_facts_stage_output: RefactorEffectFactsStageOutput,
) -> Result<RefactorEffectLoweredStageOutput, crate::effect_lowered::EffectLoweringError> {
    // P5 -> P6 canonical handoff contract：
    // - 输入必须是 P4 的 authoritative `RefactorEffectFactsStageOutput`；
    // - 输出中的 `LateLoweredProgram` / types / state graph / frame schema / dynamic invoke /
    //   resume interface / continuation object definitions 构成 P6 唯一允许消费的中层输入；
    // - P6 只能把这些 late-lowered structures 翻译到 LLVM，不得重新做 boundary 识别、
    //   whole-function segmentation、frame lifting、continuation capture 合同设计或 `ImplPlan`
    //   选择；
    // - LLVM 物理布局、ABI 与 runtime 集成仍属于 P6，而不是在 P5 回填。
    let dispatcher = dispatcher_for_session(session).late_lowering();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => {
            entry.delegate_to_legacy(|| effect_lowering_stage::run(effect_facts_stage_output))
        }
        StageEntry::Refactor(entry) => {
            entry.lower_effect_lowered_stage_output(effect_facts_stage_output)
        }
    }
}

pub fn load_effect_lowered_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<RefactorEffectLoweredStageOutput, crate::effect_lowered::EffectLoweringError> {
    let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(session, source)?;
    build_effect_lowered_stage_output(session, effect_facts_stage_output)
}

pub fn materialize_direct_style_mir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::mir::MaterializedMir, Box<crate::mir::MirMaterializeError>> {
    enter_direct_style_mir_stage(session, || {
        crate::mir::materialize_for_dump(session, source)
    })
}

#[cfg(feature = "llvm")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlvmArtifactKind {
    LlvmIr,
    Object,
    Asm,
}

#[cfg(feature = "llvm")]
pub fn emit_single_file_llvm_artifact_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    artifact: LlvmArtifactKind,
) -> Result<(), crate::llvm::LlvmEmitError> {
    enter_llvm_codegen_stage(session, || match artifact {
        LlvmArtifactKind::LlvmIr => {
            crate::llvm::emit_minimal_main_ir_to_file(session, source, output)
        }
        LlvmArtifactKind::Object => {
            crate::llvm::emit_minimal_main_obj_to_file(session, source, output)
        }
        LlvmArtifactKind::Asm => {
            crate::llvm::emit_minimal_main_asm_to_file(session, source, output)
        }
    })
}

#[cfg(feature = "llvm")]
#[allow(clippy::too_many_arguments)]
pub fn emit_production_llvm_artifact_to_file(
    session: &Session,
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: crate::hir::LoweredHir,
    abi_visibility_lowered: Option<crate::hir::LoweredHir>,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: crate::opt::OptLevel,
    artifact: LlvmArtifactKind,
) -> Result<(), crate::llvm::LlvmEmitError> {
    let dispatcher = dispatcher_for_session(session).llvm_codegen();
    match dispatcher.entry {
        StageEntry::Legacy(entry) => entry.delegate_to_legacy(|| match artifact {
            LlvmArtifactKind::LlvmIr => crate::llvm::emit_minimal_main_ir_to_file_from_production_lowered_hir_with_entry_with_opt_level(
                source_map,
                entry_source_id,
                &lowered,
                output,
                entry_main_fqn,
                opt_level,
            ),
            LlvmArtifactKind::Object => crate::llvm::emit_minimal_main_obj_to_file_from_production_lowered_hir_with_entry_with_opt_level(
                source_map,
                entry_source_id,
                &lowered,
                output,
                entry_main_fqn,
                opt_level,
            ),
            LlvmArtifactKind::Asm => crate::llvm::emit_minimal_main_asm_to_file_from_production_lowered_hir_with_entry_with_opt_level(
                source_map,
                entry_source_id,
                &lowered,
                output,
                entry_main_fqn,
                opt_level,
            ),
        }),
        StageEntry::Refactor(_) => llvm_codegen_stage::emit_artifact_to_file(
            session,
            llvm_codegen_stage::RefactorLlvmCodegenStageInput::new(
                lowered,
                abi_visibility_lowered,
                source_map.clone(),
                entry_source_id,
                entry_main_fqn.map(str::to_owned),
                opt_level,
            ),
            output,
            artifact,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};

    fn session_for(mode: EffectPipelineMode) -> Session {
        Session::with_options(SessionOptions::new(mode)).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual("<mem>", "package sample\nfun main() {}")
    }

    #[test]
    fn legacy_dispatcher_can_construct_all_stage_entries() {
        let session = session_for(EffectPipelineMode::Legacy);
        let dispatcher = dispatcher_for_session(&session);

        assert_eq!(dispatcher.mode(), EffectPipelineMode::Legacy);
        assert_eq!(dispatcher.ast().mode(), EffectPipelineMode::Legacy);
        assert_eq!(dispatcher.typed_hir().stage(), StageKind::TypedHir);
        assert_eq!(
            dispatcher.direct_style_mir().stage(),
            StageKind::DirectStyleMir
        );
        assert_eq!(dispatcher.effect_facts().stage(), StageKind::EffectFacts);
        assert_eq!(dispatcher.late_lowering().stage(), StageKind::LateLowering);
        assert_eq!(dispatcher.llvm_codegen().stage(), StageKind::LlvmCodegen);
    }

    #[test]
    fn refactor_dispatcher_can_delegate_ast_hir_and_mir_stages() {
        let session = session_for(EffectPipelineMode::Refactor);
        let source = sample_source();

        let ast_output = load_ast_stage_output_for_dump(&session, &source).unwrap();
        let hir = lower_typed_hir_for_dump(&session, &source).unwrap();
        let mir = lower_direct_style_mir_for_dump(&session, &source).unwrap();

        assert!(std::ptr::eq(ast_output.source(), &source));
        assert!(ast_output.ast().package.is_some());
        assert_eq!(hir.file.items.len(), 1);
        assert_eq!(mir.file.items.len(), 1);
        assert_eq!(
            dispatcher_for_session(&session).ast().mode(),
            EffectPipelineMode::Refactor
        );
    }

    #[test]
    fn refactor_typed_hir_stage_dispatcher_loads_stage_output() {
        let session = session_for(EffectPipelineMode::Refactor);
        let source = sample_source();

        let output = load_typed_hir_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(
            dispatcher_for_session(&session).typed_hir().mode(),
            EffectPipelineMode::Refactor
        );
        assert_eq!(output.hir_file().items.len(), 1);
        assert!(!output.effect_contracts().is_placeholder());
    }

    #[test]
    fn refactor_direct_mir_stage_dispatcher_loads_stage_output() {
        let session = session_for(EffectPipelineMode::Refactor);
        let source = sample_source();

        let output = load_direct_style_mir_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(
            dispatcher_for_session(&session).direct_style_mir().mode(),
            EffectPipelineMode::Refactor
        );
        assert_eq!(output.file().items.len(), 1);
        assert!(output.callable_body("sample.main").is_some());
    }

    #[test]
    fn refactor_effect_facts_stage_dispatcher_loads_stage_output() {
        let session = session_for(EffectPipelineMode::Refactor);
        let source = sample_source();

        let output = load_effect_facts_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(
            dispatcher_for_session(&session).effect_facts().mode(),
            EffectPipelineMode::Refactor
        );
        assert_eq!(output.file().items.len(), 1);
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.materialized_pass_view().len()
        );
    }

    #[test]
    fn refactor_effect_lowered_stage_dispatcher_loads_stage_output() {
        let session = session_for(EffectPipelineMode::Refactor);
        let source = sample_source();

        let output = load_effect_lowered_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(
            dispatcher_for_session(&session).late_lowering().mode(),
            EffectPipelineMode::Refactor
        );
        assert_eq!(
            output.program().len(),
            output.materialized_pass_view().len()
        );
        assert!(output.program().callable("sample.main").is_some());
    }
}
