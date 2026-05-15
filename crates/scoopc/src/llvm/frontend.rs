use crate::hir;
use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};

use super::LlvmEmitError;

pub(super) struct SingleFileCodegenUnit {
    pub(super) lowered: hir::LoweredHir,
    pub(super) source_map: SourceMap,
    pub(super) entry_source_id: SourceId,
}

/// 为 `emit_minimal_main_ir` / `emit_minimal_main_obj_to_file` 准备“virtual cone + support sources”
/// 的完整 codegen 输入。
///
/// 约定：
/// - 不再保留独立的“单文件前端”实现；
/// - 当前 helper 只负责把单文件输入包装成“默认 project 设置下、只含一个用户源文件的 virtual cone”，
///   然后复用 `crate::frontend` 的共享 project frontend；
/// - support sources / request roots / entry-main 选择 / HIR lowering 与显式 cone build 走同一主线；
/// - 返回的 `lowered` 仍承载当前 LLVM codegen 需要的 HIR 兼容输入，但会保留
///   `LoweredHir::materialized_pass_view()`，作为 production 主路径显式接入的 canonical
///   materialized body / summary / 后续 MIR pass 产物视图。
#[cfg(test)]
pub(super) fn prepare_single_file_codegen_unit(
    session: &Session,
    entry_source: &SourceFile,
) -> Result<SingleFileCodegenUnit, LlvmEmitError> {
    prepare_single_file_codegen_unit_with_opt_level(session, entry_source, OptLevel::O0)
}

pub(super) fn prepare_single_file_codegen_unit_with_opt_level(
    session: &Session,
    entry_source: &SourceFile,
    opt_level: OptLevel,
) -> Result<SingleFileCodegenUnit, LlvmEmitError> {
    let context = crate::frontend::prepare_virtual_cone_context_with_options(
        entry_source.clone(),
        session.options(),
    )
    .map_err(frontend_error)?;
    let front = crate::frontend::run_project_frontend(session, context).map_err(frontend_error)?;
    let lowered = crate::frontend::lower_hir_for_codegen_with_request_root_mode(
        session,
        &front,
        opt_level,
        crate::frontend::MirRequestRootMode::EntryMain,
    )
    .map_err(frontend_error)?;
    let (source_map, entry_source_id) = crate::frontend::build_source_map(session, front.input());

    Ok(SingleFileCodegenUnit {
        lowered,
        source_map,
        entry_source_id,
    })
}

fn frontend_error(error: impl std::fmt::Display) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: error.to_string(),
    }
}

#[cfg(test)]
pub(super) fn build_source_map_with_extra_sources(
    session: &Session,
    input_sources: &[SourceFile],
    entry_index: usize,
) -> (SourceMap, SourceId) {
    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }

    let mut entry_source_id = None;
    for (idx, source) in input_sources.iter().enumerate() {
        let source_id = source_map.add_source_clone(source);
        if idx == entry_index {
            entry_source_id = Some(source_id);
        }
    }

    (
        source_map,
        entry_source_id.expect("entry source should always be present in source map"),
    )
}
