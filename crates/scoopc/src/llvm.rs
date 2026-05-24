use std::path::Path;

use inkwell::context::Context;

pub use scoopc_codegen_llvm::llvm::*;

use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::SourceFile;

fn frontend_error(error: impl std::fmt::Display) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: error.to_string(),
    }
}

fn build_single_file_stage_output(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> Result<LlvmCodegenStageOutput, LlvmEmitError> {
    let context = crate::frontend::prepare_virtual_cone_context_with_options(
        source.clone(),
        session.options(),
    )
    .map_err(frontend_error)?;
    let front = crate::frontend::run_project_frontend(session, context).map_err(frontend_error)?;
    let lowering = crate::frontend::lower_hir_for_codegen_with_request_root_mode(
        session,
        &front,
        opt_level,
        crate::frontend::MirRequestRootMode::EntryMain,
    )
    .map_err(frontend_error)?;
    let (source_map, entry_source_id) = crate::frontend::build_source_map(session, front.input());

    crate::pipeline::run_llvm_codegen_stage(
        session,
        crate::pipeline::LlvmCodegenStageInput::new(
            lowering,
            None,
            source_map,
            entry_source_id,
            None,
            opt_level,
        ),
    )
}

pub(crate) fn emit_single_file_llvm_artifact_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    artifact: LlvmArtifactKind,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, opt_level)?;
    let stage_input = StageEmitInput::from_stage_output(&stage_output);
    match artifact {
        LlvmArtifactKind::LlvmIr => emit_main_ir_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Object => emit_main_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Asm => emit_main_asm_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
    }
}

pub fn emit_minimal_main_ir(
    session: &Session,
    source: &SourceFile,
) -> Result<String, LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, OptLevel::O0)?;
    let context = Context::create();
    let module = build_main_module_from_stage_output(
        stage_output.source_map(),
        stage_output.entry_source_id(),
        &context,
        StageEmitInput::from_stage_output(&stage_output),
        None,
    )?;
    Ok(module.print_to_string().to_string())
}

pub fn emit_minimal_main_ir_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir(session, source)?;
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })
}

pub fn emit_minimal_main_obj_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

pub fn emit_minimal_main_obj_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, opt_level)?;
    emit_main_obj_to_file_from_stage_output(
        stage_output.source_map(),
        stage_output.entry_source_id(),
        StageEmitInput::from_stage_output(&stage_output),
        output,
        None,
        opt_level,
    )
}

pub fn emit_minimal_main_asm_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

pub fn emit_minimal_main_asm_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, opt_level)?;
    emit_main_asm_to_file_from_stage_output(
        stage_output.source_map(),
        stage_output.entry_source_id(),
        StageEmitInput::from_stage_output(&stage_output),
        output,
        None,
        opt_level,
    )
}
