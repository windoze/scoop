//! 统一 pipeline 的顶层 stage API。
//!
//! P8 起，legacy/selector 与并行 dispatcher 已删除；本模块只暴露当前唯一
//! 生效的 production stage 入口，不再承载任何双主线分发语义。

mod ast_stage;
mod effect_facts_stage;
mod effect_lowering_stage;
mod hir_completeness;
#[cfg(test)]
mod hir_preflight;
mod hir_stage;
#[cfg(feature = "llvm")]
mod llvm_codegen_stage;
mod mir_stage;

use crate::session::Session;
use crate::source::SourceFile;

pub use ast_stage::AstStageOutput;
pub use effect_facts_stage::EffectFactsStageOutput;
pub use effect_lowering_stage::EffectLoweredStageOutput;
pub use hir_stage::{
    CallArgBindingContract, CallArgElementContract, CallArgParamContract,
    ConstructorCallTargetContract, ContinuationResumeReceiverRoute, ContinuationResumeSiteContract,
    ExternGlobalContract, FunctionEffectContract, FunctionTargetContract, HandleArmContractKind,
    HandleArmSiteContract, HandleSiteContract, MemberCallTargetContract, PayloadTypeContract,
    PerformSiteContract, TopLevelInitDependency, TopLevelInitDependencyKind,
    TopLevelInitRootContract, TopLevelInitRootKind, TypedCallSiteContract, TypedCallSiteKind,
    TypedHirEffectContracts, TypedHirStageOutput, TypedIntrinsicKind,
};
#[cfg(feature = "llvm")]
pub use llvm_codegen_stage::{LlvmCodegenStageInput, LlvmCodegenStageOutput};
pub use mir_stage::MirStageOutput;

#[cfg(feature = "llvm")]
use crate::opt::OptLevel;
#[cfg(feature = "llvm")]
use crate::source::{SourceId, SourceMap};
#[cfg(feature = "llvm")]
use std::path::Path;

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
    ast_stage::run(session, source)
}

pub fn load_typed_hir_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, crate::hir::HirLowerError> {
    hir_stage::run(session, source)
}

pub fn lower_typed_hir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::hir::LoweredHir, crate::hir::HirLowerError> {
    load_typed_hir_stage_output_for_dump(session, source).map(TypedHirStageOutput::into_lowered_hir)
}

pub fn lower_direct_style_mir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::mir::LoweredMir, crate::mir::MirLowerError> {
    load_direct_style_mir_stage_output_for_dump(session, source)
        .map(MirStageOutput::into_lowered_mir)
}

pub fn load_direct_style_mir_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<MirStageOutput, crate::mir::MirLowerError> {
    let typed_hir_output = load_typed_hir_stage_output_for_dump(session, source)
        .map_err(crate::mir::MirLowerError::from)?;
    mir_stage::run(typed_hir_output)
}

pub fn build_effect_facts_stage_output(
    session: &Session,
    source: &SourceFile,
    mir_stage_output: MirStageOutput,
) -> Result<EffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
    build_effect_facts_stage_output_with_compilation_sources(
        session,
        source,
        std::slice::from_ref(source),
        mir_stage_output,
    )
}

pub(crate) fn build_effect_facts_stage_output_with_compilation_sources(
    session: &Session,
    source: &SourceFile,
    compilation_sources: &[SourceFile],
    mir_stage_output: MirStageOutput,
) -> Result<EffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
    // P4 facts 必须绑定到 canonical materialized MIR snapshot。
    // 当前 P3 dump stage 仍允许在未保留 snapshot 的情况下独立产出 direct-style MIR，
    // 因此在 effect-facts stage 边界用同一 session/source 路由补挂 canonical snapshot。
    let mir_stage_output = if mir_stage_output.materialized_mir().is_some() {
        mir_stage_output
    } else {
        let materialized = materialize_direct_style_mir_for_dump(session, source)?;
        mir_stage_output.with_materialized_mir(materialized)
    };
    effect_facts_stage::run_with_compilation_sources(
        session,
        source,
        compilation_sources,
        mir_stage_output,
    )
}

pub fn load_effect_facts_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<EffectFactsStageOutput, crate::effect_facts::EffectFactsError> {
    let mir_stage_output = load_direct_style_mir_stage_output_for_dump(session, source)
        .map_err(crate::effect_facts::EffectFactsError::from)?;
    build_effect_facts_stage_output(session, source, mir_stage_output)
}

pub fn build_effect_lowered_stage_output(
    session: &Session,
    effect_facts_stage_output: EffectFactsStageOutput,
) -> Result<EffectLoweredStageOutput, crate::effect_lowered::EffectLoweringError> {
    // P5 -> P6 canonical handoff contract：
    // - 输入必须是 P4 的 authoritative `EffectFactsStageOutput`；
    // - 输出中的 `LateLoweredProgram` / types / state graph / frame schema / dynamic invoke /
    //   authoritative per-op/per-schema resume publication（step cases、continuation object、
    //   surface-resume dispatch inventory）以及可选的 effect-family resume packing definitions
    //   构成 P6 唯一允许消费的中层输入；
    // - P6 只能把这些 late-lowered structures 翻译到 LLVM，不得重新做 boundary 识别、
    //   whole-function segmentation、frame lifting、continuation capture 合同设计或 `ImplPlan`
    //   选择，也不得把 packing layer 重新提升为 reverse-resume 语义主键；
    // - LLVM 物理布局、ABI 与 runtime 集成仍属于 P6，而不是在 P5 回填。
    let _ = session;
    effect_lowering_stage::run(effect_facts_stage_output)
}

pub fn load_effect_lowered_stage_output_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<EffectLoweredStageOutput, crate::effect_lowered::EffectLoweringError> {
    let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(session, source)?;
    build_effect_lowered_stage_output(session, effect_facts_stage_output)
}

pub fn materialize_direct_style_mir_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<crate::mir::MaterializedMir, Box<crate::mir::MirMaterializeError>> {
    crate::mir::materialize_for_dump(session, source)
}

#[cfg(feature = "llvm")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlvmArtifactKind {
    LlvmIr,
    Object,
    Asm,
}

#[cfg(feature = "llvm")]
pub(crate) fn run_llvm_codegen_stage(
    session: &Session,
    input: LlvmCodegenStageInput,
) -> Result<LlvmCodegenStageOutput, crate::llvm::LlvmEmitError> {
    llvm_codegen_stage::run(session, input)
}

#[cfg(feature = "llvm")]
/// 以 single-source virtual-cone contract 发射 LLVM artifact。
///
/// 该入口只接受裸 `SourceFile` 语义；若调用方拥有显式 cone / 多源 project context，
/// 应改用 `emit_project_llvm_artifact_to_file(...)`，而不是让末端 helper 猜目录语义。
pub fn emit_virtual_cone_llvm_artifact_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    artifact: LlvmArtifactKind,
) -> Result<(), crate::llvm::LlvmEmitError> {
    crate::llvm::emit_single_file_llvm_artifact_to_file_with_opt_level(
        session,
        source,
        output,
        artifact,
        OptLevel::O0,
    )
}

#[cfg(feature = "llvm")]
/// 以 authoritative project context（`FrontendOutput`）发射 LLVM artifact。
///
/// 这条入口对应 `scoop` -> `scoopc` 的 project build contract：上层驱动负责先确定
/// `ProjectInput + deps`，`scoopc` 负责消费完整 context 运行 frontend/lowering/codegen。
pub fn emit_project_llvm_artifact_to_file(
    session: &Session,
    front: &crate::frontend::FrontendOutput,
    output: &Path,
    opt_level: crate::opt::OptLevel,
    artifact: LlvmArtifactKind,
) -> Result<Vec<String>, crate::llvm::LlvmEmitError> {
    let lowered = crate::frontend::lower_hir_for_codegen_with_request_root_mode(
        session,
        front,
        opt_level,
        crate::frontend::MirRequestRootMode::EntryMain,
    )
    .map_err(project_frontend_prepare_error)?;
    let extern_libs = lowered.extern_libs.clone();
    let abi_visibility_lowered = crate::frontend::lower_hir_for_codegen_with_request_root_mode(
        session,
        front,
        opt_level,
        crate::frontend::MirRequestRootMode::RequestSources,
    )
    .map(Some)
    .map_err(project_frontend_prepare_error)?;
    let (source_map, entry_source_id) = crate::frontend::build_source_map(session, front.input());
    emit_production_llvm_artifact_to_file(
        session,
        &source_map,
        entry_source_id,
        lowered,
        abi_visibility_lowered,
        output,
        front.input().entry_main_fqn(),
        opt_level,
        artifact,
    )?;
    Ok(extern_libs)
}

#[cfg(feature = "llvm")]
fn project_frontend_prepare_error(error: impl std::fmt::Display) -> crate::llvm::LlvmEmitError {
    crate::llvm::LlvmEmitError::Frontend {
        message: error.to_string(),
    }
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
    llvm_codegen_stage::emit_artifact_to_file(
        session,
        llvm_codegen_stage::LlvmCodegenStageInput::new(
            lowered,
            abi_visibility_lowered,
            source_map.clone(),
            entry_source_id,
            entry_main_fqn.map(str::to_owned),
            opt_level,
        ),
        output,
        artifact,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, SessionOptions};

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual("<mem>", "package sample\nfun main() {}")
    }

    #[test]
    fn single_pipeline_loads_ast_hir_and_mir_stages() {
        let session = session();
        let source = sample_source();

        let ast_output = load_ast_stage_output_for_dump(&session, &source).unwrap();
        let hir = lower_typed_hir_for_dump(&session, &source).unwrap();
        let mir = lower_direct_style_mir_for_dump(&session, &source).unwrap();

        assert!(std::ptr::eq(ast_output.source(), &source));
        assert!(ast_output.ast().package.is_some());
        assert_eq!(hir.file.items.len(), 1);
        assert_eq!(mir.file.items.len(), 1);
    }

    #[test]
    fn single_pipeline_typed_hir_stage_loads_stage_output() {
        let session = session();
        let source = sample_source();

        let output = load_typed_hir_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(output.hir_file().items.len(), 1);
        assert!(!output.effect_contracts().is_placeholder());
    }

    #[test]
    fn single_pipeline_direct_mir_stage_loads_stage_output() {
        let session = session();
        let source = sample_source();

        let output = load_direct_style_mir_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(output.file().items.len(), 1);
        assert!(output.callable_body("sample.main").is_some());
    }

    #[test]
    fn single_pipeline_effect_facts_stage_loads_stage_output() {
        let session = session();
        let source = sample_source();

        let output = load_effect_facts_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(output.file().items.len(), 1);
        assert_eq!(
            output.effect_facts().callable_facts().len(),
            output.materialized_pass_view().len()
        );
    }

    #[test]
    fn single_pipeline_effect_lowered_stage_loads_stage_output() {
        let session = session();
        let source = sample_source();

        let output = load_effect_lowered_stage_output_for_dump(&session, &source).unwrap();

        assert_eq!(
            output.program().len(),
            output.materialized_pass_view().len()
        );
        assert!(output.program().callable("sample.main").is_some());
    }
}
