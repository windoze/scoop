//! LLVM emit API 与 module build 入口。
//!
//! 这层负责：
//! - 面向外部的 `emit_minimal_main_*` API；
//! - 把 `hir::LoweredHir` 组装成单个 LLVM module；
//! - 在进入 backend lowering 前完成 reachability 与 eager inclusion。
//!
//! 它不负责定义 LLVM pass pipeline，也不在根模块中继续承载大段实现。
//! P8-T03a 起，默认单文件 `emit_minimal_main_*` / `build_minimal_main_module*` 统一经 refactor
//! LLVM stage handoff 构建；显式 `*_from_lowered_hir` / `*_from_materialized_lowered_hir` helper
//! 仅保留给测试、对照和历史桥接调用方，不能再作为默认生产单文件入口。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use inkwell::context::Context;
use inkwell::targets::{FileType, TargetData};

use crate::hir;
use crate::opt::OptLevel;
use crate::program_facts::ProgramFacts;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::ty::{RefTypeKind, TypeKind, ValueTypeKind};

use super::frontend;
use super::pipeline::run_pass_pipeline;
use super::reachability::{ReachabilityInputs, collect_reachable_top_level_funs};
use super::{LlvmEmitError, codegen, configure_llvm_global_options_once, target};

struct LoweredCodegenEntry<'a> {
    lowered: &'a hir::LoweredHir,
    materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    codegen_routing_facts: Option<&'a crate::mir::MirCodegenRoutingFacts>,
    late_lowered_program: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    late_lowered_types: Option<&'a crate::ty::TypeStore>,
    refactor_abi_program: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    refactor_abi_types: Option<&'a crate::ty::TypeStore>,
    refactor_abi_materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    refactor_abi_effect_facts: Option<&'a crate::effect_facts::MaterializedEffectFacts>,
}

#[derive(Clone, Copy)]
pub struct RefactorStageEmitInput<'a> {
    hir_compat_scaffold: &'a hir::LoweredHir,
    effect_lowered_stage_output:
        &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
    abi_visibility_effect_lowered_stage_output:
        Option<&'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput>,
}

impl<'a> RefactorStageEmitInput<'a> {
    pub fn new(
        hir_compat_scaffold: &'a hir::LoweredHir,
        effect_lowered_stage_output: &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<
            &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        >,
    ) -> Self {
        Self {
            hir_compat_scaffold,
            effect_lowered_stage_output,
            abi_visibility_effect_lowered_stage_output,
        }
    }
}

fn build_single_file_refactor_stage_output(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> Result<crate::effect_refactor_pipeline::RefactorLlvmCodegenStageOutput, LlvmEmitError> {
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(session, source, opt_level)?;
    crate::effect_refactor_pipeline::run_llvm_codegen_stage(
        session,
        crate::effect_refactor_pipeline::RefactorLlvmCodegenStageInput::new(
            codegen_unit.lowered,
            None,
            codegen_unit.source_map,
            codegen_unit.entry_source_id,
            None,
            opt_level,
        ),
    )
}

pub(crate) fn emit_single_file_llvm_artifact_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    artifact: crate::effect_refactor_pipeline::LlvmArtifactKind,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let stage_output = build_single_file_refactor_stage_output(session, source, opt_level)?;
    let stage_input = RefactorStageEmitInput::new(
        stage_output.hir_compat_scaffold(),
        stage_output.effect_lowered_stage_output(),
        stage_output.abi_visibility_effect_lowered_stage_output(),
    );
    match artifact {
        crate::effect_refactor_pipeline::LlvmArtifactKind::LlvmIr => {
            emit_refactor_main_ir_to_file_from_stage_output(
                stage_output.source_map(),
                stage_output.entry_source_id(),
                stage_input,
                output,
                stage_output.entry_main_fqn(),
                stage_output.opt_level(),
            )
        }
        crate::effect_refactor_pipeline::LlvmArtifactKind::Object => {
            emit_refactor_main_obj_to_file_from_stage_output(
                stage_output.source_map(),
                stage_output.entry_source_id(),
                stage_input,
                output,
                stage_output.entry_main_fqn(),
                stage_output.opt_level(),
            )
        }
        crate::effect_refactor_pipeline::LlvmArtifactKind::Asm => {
            emit_refactor_main_asm_to_file_from_stage_output(
                stage_output.source_map(),
                stage_output.entry_source_id(),
                stage_input,
                output,
                stage_output.entry_main_fqn(),
                stage_output.opt_level(),
            )
        }
    }
}

impl<'a> LoweredCodegenEntry<'a> {
    fn from_lowered_hir(lowered: &'a hir::LoweredHir) -> Self {
        Self {
            lowered,
            materialized_pass_view: lowered.materialized_pass_view(),
            codegen_routing_facts: None,
            late_lowered_program: None,
            late_lowered_types: None,
            refactor_abi_program: None,
            refactor_abi_types: None,
            refactor_abi_materialized_pass_view: None,
            refactor_abi_effect_facts: None,
        }
    }

    fn from_materialized_lowered_hir(lowered: &'a hir::LoweredHir) -> Result<Self, LlvmEmitError> {
        let materialized_pass_view = lowered
            .materialized_pass_view()
            .ok_or(LlvmEmitError::MissingMaterializedPassView)?;
        Ok(Self {
            lowered,
            materialized_pass_view: Some(materialized_pass_view),
            codegen_routing_facts: None,
            late_lowered_program: None,
            late_lowered_types: None,
            refactor_abi_program: None,
            refactor_abi_types: None,
            refactor_abi_materialized_pass_view: None,
            refactor_abi_effect_facts: None,
        })
    }

    fn from_refactor_stage(
        lowered: &'a hir::LoweredHir,
        effect_lowered_stage_output: &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<
            &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        >,
    ) -> Self {
        debug_assert!(
            lowered.materialized_pass_view().is_none(),
            "refactor LLVM stage 的 HIR scaffold 不应继续携带旧 production pass-view"
        );
        let abi_visibility_effect_lowered_stage_output =
            abi_visibility_effect_lowered_stage_output.unwrap_or(effect_lowered_stage_output);
        Self {
            lowered,
            materialized_pass_view: Some(effect_lowered_stage_output.materialized_pass_view()),
            codegen_routing_facts: Some(effect_lowered_stage_output.codegen_routing_facts()),
            late_lowered_program: Some(effect_lowered_stage_output.program()),
            late_lowered_types: Some(effect_lowered_stage_output.types()),
            refactor_abi_program: Some(abi_visibility_effect_lowered_stage_output.program()),
            refactor_abi_types: Some(abi_visibility_effect_lowered_stage_output.types()),
            refactor_abi_materialized_pass_view: Some(
                abi_visibility_effect_lowered_stage_output.materialized_pass_view(),
            ),
            refactor_abi_effect_facts: Some(
                abi_visibility_effect_lowered_stage_output.effect_facts(),
            ),
        }
    }
}

/// 为一个 Scoop 程序生成默认单文件 LLVM IR（`.ll` 文本）。
///
/// 当前默认路径会先运行 refactor LLVM stage，再消费其 authoritative handoff 发射产物；
/// 仅显式 `*_from_materialized_lowered_hir` helper 仍保留 raw materialized-MIR 对照语义。
///
/// 输出形态：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple / data layout 设为 host；
/// - `i32 @main(i32 argc, i8** argv)` 的 body 来自 `fun main` 的 v1 子集 codegen；若 `main` 为空则返回 0。
pub fn emit_minimal_main_ir(
    session: &Session,
    source: &SourceFile,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_minimal_main_module_with_opt_level(session, source, &context, OptLevel::O0)?;
    Ok(module.print_to_string().to_string())
}

/// 基于“已完成 resolver 的 AST lowering 结果”（`hir::LoweredHir`）生成 LLVM IR。
///
/// 用途（T1107）：
/// - `scoop build` 在多包（cone 依赖）场景下，需要复用同一套“已注入 `.cone` 依赖”的编译单元，
///   避免后端再次独立 parse/resolve 导致 import 失败或语义分叉。
pub fn emit_minimal_main_ir_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_lowered_hir(lowered),
        None,
    )?;
    Ok(module.print_to_string().to_string())
}

/// 基于 production frontend 保留的 canonical materialized MIR/pass 视图生成 LLVM IR。
///
/// 该入口仅保留给显式历史/对照测试；默认单文件 production 入口不再间接调用它。
///
/// 该入口要求 `lowered` 显式携带 `LoweredHir::materialized_pass_view()`；
/// 若调用方只提供不带 canonical pass view 的测试 lowering，则返回结构化错误，而不是静默回退到只看 HIR
/// 兼容 body。
pub fn emit_minimal_main_ir_from_materialized_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_materialized_lowered_hir(lowered)?,
        None,
    )?;
    Ok(module.print_to_string().to_string())
}

/// 生成最小 LLVM IR，并写入到指定路径（通常为 `.ll`）。
pub fn emit_minimal_main_ir_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir(session, source)?;

    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（通常为 `.ll`）。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir_from_lowered_hir(source_map, entry_source_id, lowered)?;
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;
    let ir = module.print_to_string().to_string();

    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
///
/// 与 `emit_minimal_main_ir_to_file_from_lowered_hir_with_entry` 的区别：
/// - 该版本会按 `opt_level` 运行 LLVM PassBuilder pipeline（包含 statepoint 重写），确保 `--emit-llvm`
///   的输出能反映优化等级差异，便于 build fixtures 断言与回归。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;

    let ir = module.print_to_string().to_string();
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 production frontend 保留的 canonical materialized MIR/pass 视图生成 LLVM IR，并写入到指定路径。
///
/// 该入口仅保留给显式历史/对照测试；默认单文件 production 入口不再间接调用它。
pub fn emit_minimal_main_ir_to_file_from_materialized_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_materialized_lowered_hir(lowered)?,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;

    let ir = module.print_to_string().to_string();
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 refactor LLVM stage handoff（P5 late-lowered output + HIR compatibility scaffold）构建 LLVM module。
pub(crate) fn build_refactor_main_module_from_stage_output<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    stage_input: RefactorStageEmitInput<'_>,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        context,
        LoweredCodegenEntry::from_refactor_stage(
            stage_input.hir_compat_scaffold,
            stage_input.effect_lowered_stage_output,
            stage_input.abi_visibility_effect_lowered_stage_output,
        ),
        entry_main_fqn,
    )
}

/// 基于 refactor LLVM stage handoff 生成 LLVM IR，并写入到指定路径。
pub fn emit_refactor_main_ir_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: RefactorStageEmitInput<'_>,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_refactor_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;

    let ir = module.print_to_string().to_string();
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

/// 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module_with_opt_level(session, source, &context, opt_level)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_from_lowered_hir_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module =
        build_main_module_from_lowered_hir(source_map, entry_source_id, &context, lowered, None)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        entry_main_fqn,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 production frontend 保留的 canonical materialized MIR/pass 视图生成最小 LLVM object。
///
/// 该入口仅保留给显式历史/对照测试；默认单文件 production 入口不再间接调用它。
pub fn emit_minimal_main_obj_to_file_from_materialized_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_materialized_lowered_hir(lowered)?,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 refactor LLVM stage handoff 生成 LLVM object，并写入到指定路径。
pub fn emit_refactor_main_obj_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: RefactorStageEmitInput<'_>,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_refactor_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

/// 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module_with_opt_level(session, source, &context, opt_level)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_from_lowered_hir_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module =
        build_main_module_from_lowered_hir(source_map, entry_source_id, &context, lowered, None)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        entry_main_fqn,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 production frontend 保留的 canonical materialized MIR/pass 视图生成最小 LLVM assembly。
///
/// 该入口仅保留给显式历史/对照测试；默认单文件 production 入口不再间接调用它。
pub fn emit_minimal_main_asm_to_file_from_materialized_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_materialized_lowered_hir(lowered)?,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 refactor LLVM stage handoff 生成 LLVM assembly，并写入到指定路径。
pub fn emit_refactor_main_asm_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: RefactorStageEmitInput<'_>,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_refactor_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn build_minimal_main_module<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_minimal_main_module_with_opt_level(session, source, context, OptLevel::O0)
}

pub(crate) fn build_minimal_main_module_with_opt_level<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
    opt_level: OptLevel,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    let stage_output = build_single_file_refactor_stage_output(session, source, opt_level)?;
    build_refactor_main_module_from_stage_output(
        stage_output.source_map(),
        stage_output.entry_source_id(),
        context,
        RefactorStageEmitInput::new(
            stage_output.hir_compat_scaffold(),
            stage_output.effect_lowered_stage_output(),
            stage_output.abi_visibility_effect_lowered_stage_output(),
        ),
        None,
    )
}

pub(crate) fn build_main_module_from_lowered_hir<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    lowered: &hir::LoweredHir,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        context,
        LoweredCodegenEntry::from_lowered_hir(lowered),
        RootCallableSelector::EntryMain { entry_main_fqn },
    )
}

fn build_main_module_from_codegen_entry<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    codegen_entry: LoweredCodegenEntry<'_>,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        context,
        codegen_entry,
        RootCallableSelector::EntryMain { entry_main_fqn },
    )
}

#[cfg(test)]
pub(crate) fn emit_materialized_ir_for_root_callable(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    callable_fqn: &str,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        &context,
        LoweredCodegenEntry::from_materialized_lowered_hir(lowered)?,
        RootCallableSelector::Callable { callable_fqn },
    )?;
    Ok(module.print_to_string().to_string())
}

fn build_module_from_codegen_entry_with_root_selector<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    codegen_entry: LoweredCodegenEntry<'_>,
    root_selector: RootCallableSelector<'_>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    configure_llvm_global_options_once();

    let LoweredCodegenEntry {
        lowered,
        materialized_pass_view,
        codegen_routing_facts,
        late_lowered_program,
        late_lowered_types,
        refactor_abi_program,
        refactor_abi_types,
        refactor_abi_materialized_pass_view,
        refactor_abi_effect_facts,
    } = codegen_entry;
    let has_materialized_pass_view = materialized_pass_view.is_some();

    let entry_source = entry_source(source_map, entry_source_id);
    let module_name = module_name_from_path(entry_source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;
    let target_data = TargetData::create(&target_info.data_layout);

    let selected_root = select_root_callable(lowered, root_selector)?;
    let root_fun = selected_root.fun;
    if let Some(program) = late_lowered_program
        && program.callable(&root_fun.fqn).is_none()
    {
        return Err(LlvmEmitError::Frontend {
            message: format!(
                "refactor LLVM stage handoff 缺少入口 callable `{}` 的 late-lowered body",
                root_fun.fqn
            ),
        });
    }
    let builder = context.create_builder();

    let fun_index: HashMap<String, &hir::FunDecl> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered.member_funs.iter())
        .map(|fun| (fun.fqn.clone(), fun))
        .collect();
    let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
    let effect_op_tags = Rc::new(RefCell::new(codegen::EffectOpTagState::new()));

    // T0810：在确认入口存在后，再声明/生成 `main` 可达的其它顶层函数：
    // - 避免“无 main”时把无关错误暴露给调用方；
    // - 避免因为文件里存在“当前后端不支持的函数签名”（例如泛型函数）而影响不相关的程序。
    let unit_codegen =
        codegen::CompilationUnitCodegenCx::new(codegen::CompilationUnitCodegenInputs {
            context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map,
            entry_source_id,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            extern_globals: &lowered.extern_globals,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            dispatch_call_sites: &lowered.dispatch_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            fun_index: &fun_index,
            materialized_pass_view,
            codegen_routing_facts,
            program_facts: Rc::clone(&program_facts),
            effect_op_tags: Rc::clone(&effect_op_tags),
        });
    debug_assert_eq!(
        unit_codegen.materialized_pass_view().is_some(),
        has_materialized_pass_view,
        "CompilationUnitCodegenCx 应保留 LLVM production 入口显式接入的 materialized pass view 边界"
    );
    let mut declare = unit_codegen.fresh_main_codegen();

    let mut reachable: Vec<&hir::FunDecl> = collect_reachable_top_level_funs(
        root_fun,
        &fun_index,
        unit_codegen.materialized_pass_view(),
        ReachabilityInputs {
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            extern_globals: &lowered.extern_globals,
            object_inits: &lowered.object_inits,
        },
    );

    // T0126: Eagerly include monomorphized generic class member methods.
    // When a generic class method like `Box.get` is reachable, also include all its
    // monomorphized variants (e.g., `Box.get::<Int>`, `Box.get::<String>`).
    {
        let reachable_fqns: HashSet<&str> = reachable.iter().map(|f| f.fqn.as_str()).collect();
        let mut monomorphized: Vec<&hir::FunDecl> = Vec::new();
        for (fqn, fun) in &fun_index {
            // Monomorphized member methods have `::<` in their FQN.
            if fqn.contains("::<") && !reachable_fqns.contains(fqn.as_str()) {
                // Check if the base (non-monomorphized) method is reachable.
                if let Some(base_fqn) = fqn.split("::<").next()
                    && reachable_fqns.contains(base_fqn)
                {
                    monomorphized.push(fun);
                }
            }
        }
        reachable.extend(monomorphized);
    }

    if selected_root.entry_main_arg_shape.is_none()
        && !reachable.iter().any(|fun| fun.fqn == root_fun.fqn)
    {
        reachable.push(root_fun);
    }

    // T0126: Helper to check if a function's signature contains TypeKind::Param
    // (recursively, including inside Nominal type args like `Printer<T>`).
    let ty_contains_param = |types: &crate::ty::TypeStore, ty: crate::ty::TypeId| -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match types.kind(id) {
                crate::ty::TypeKind::Param(_) => return true,
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                    stack.extend(n.args.iter().copied());
                }
                _ => {}
            }
        }
        false
    };
    let fun_has_param_types = |fun: &hir::FunDecl| -> bool {
        fun.params
            .iter()
            .any(|p| ty_contains_param(&lowered.types, p.ty))
            || ty_contains_param(&lowered.types, fun.return_ty)
    };

    reachable.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    let reachable: Vec<&hir::FunDecl> = reachable
        .into_iter()
        // T0126: Skip generic (unmonomorphized) member methods — they contain Param types
        // that cannot be lowered to LLVM types. The monomorphized variants handle these.
        .filter(|fun| !fun_has_param_types(fun))
        .collect();

    let refactor_abi_query = if let Some(program) = late_lowered_program {
        let late_lowered_types = late_lowered_types.ok_or_else(|| LlvmEmitError::Frontend {
            message: "refactor LLVM stage handoff 缺少 late-lowered TypeStore".to_string(),
        })?;
        let abi_program = refactor_abi_program.ok_or_else(|| LlvmEmitError::Frontend {
            message: "refactor LLVM stage handoff 缺少 ABI visibility late-lowered program"
                .to_string(),
        })?;
        let abi_types = refactor_abi_types.unwrap_or(late_lowered_types);
        let abi_pass_view = refactor_abi_materialized_pass_view
            .as_ref()
            .ok_or(LlvmEmitError::MissingMaterializedPassView)?;
        let abi_effect_facts =
            refactor_abi_effect_facts.ok_or_else(|| LlvmEmitError::Frontend {
                message: "refactor LLVM stage handoff 缺少 ABI visibility effect facts".to_string(),
            })?;
        let abi_query = declare.materialize_refactor_program_abi(
            abi_program,
            abi_types,
            abi_pass_view,
            abi_effect_facts,
        )?;
        let primary_pass_view = unit_codegen
            .materialized_pass_view()
            .ok_or(LlvmEmitError::MissingMaterializedPassView)?;
        declare.codegen_refactor_program_bodies(
            program,
            abi_program,
            late_lowered_types,
            primary_pass_view,
            abi_types,
            abi_pass_view,
            &abi_query,
        )?;
        Some(abi_query)
    } else {
        None
    };

    for fun in &reachable {
        let _ = declare.declare_top_level_fun(fun)?;
    }

    for fun in &reachable {
        if let Some(program) = late_lowered_program
            && let Some(callable) = program.callable(&fun.fqn)
            && (callable.plain_abi().is_some() || callable.effect_step_abi().is_some())
        {
            continue;
        }
        if !should_emit_reachable_fun_body(fun, &unit_codegen) {
            continue;
        }
        let llvm_fun = module
            .get_function(&fun.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "missing declared function",
                at: fun.span.into(),
            })?;
        let mut body_codegen = unit_codegen.fresh_main_codegen();
        if let Some(mir_fun) = canonical_materialized_callable_body(fun, &unit_codegen) {
            let body_is_overridden = unit_codegen
                .materialized_pass_view()
                .is_some_and(|view| view.callable_body_is_overridden(&fun.fqn));
            if !body_is_overridden
                && body_codegen.raw_materialized_mir_body_requires_hir_compat_boundary(fun, mir_fun)
            {
                body_codegen.codegen_top_level_fun(fun, llvm_fun)?;
            } else {
                body_codegen.codegen_top_level_mir_fun(fun, mir_fun, llvm_fun)?;
            }
        } else {
            body_codegen.codegen_top_level_fun(fun, llvm_fun)?;
        }
    }

    if let Some(arg_shape) = selected_root.entry_main_arg_shape {
        let i32_type = context.i32_type();
        let i8_ptr_ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
        let fn_type = i32_type.fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false);

        let main = module.add_function("main", fn_type, None);
        let entry = context.append_basic_block(main, "entry");
        builder.position_at_end(entry);

        let argc = main
            .get_nth_param(0)
            .ok_or(LlvmEmitError::ModuleVerificationFailed {
                message: "entry main 缺少 argc 参数".to_string(),
            })?
            .into_int_value();
        let argv = main
            .get_nth_param(1)
            .ok_or(LlvmEmitError::ModuleVerificationFailed {
                message: "entry main 缺少 argv 参数".to_string(),
            })?
            .into_pointer_value();
        argc.set_name("argc");
        argv.set_name("argv");

        let rt_init = module
            .get_function("scoop_runtime_init")
            .unwrap_or_else(|| {
                module.add_function(
                    "scoop_runtime_init",
                    context.void_type().fn_type(&[], false),
                    None,
                )
            });
        builder.build_call(rt_init, &[], "rt_init")?;

        let mut main_codegen = unit_codegen.fresh_main_codegen();
        main_codegen.begin_function_explicit_frame_layout(main)?;

        let entry_argv_array = match arg_shape {
            EntryMainArgShape::None => None,
            EntryMainArgShape::ArrayString => {
                let argv_array_fn = module
                    .get_function("scoop_entry_argv_array")
                    .unwrap_or_else(|| {
                        module.add_function(
                            "scoop_entry_argv_array",
                            context
                                .ptr_type(inkwell::AddressSpace::from(1u16))
                                .fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false),
                            None,
                        )
                    });
                let call =
                    builder.build_call(argv_array_fn, &[argc.into(), argv.into()], "entry_argv")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::ModuleVerificationFailed {
                        message: "entry argv helper 未返回值".to_string(),
                    },
                )?;
                Some(raw.into_pointer_value())
            }
        };

        let exit_code = if let (Some(program), Some(abi_query)) =
            (late_lowered_program, refactor_abi_query.as_ref())
        {
            main_codegen.codegen_refactor_main_exit_code(
                root_fun,
                entry_argv_array,
                program,
                abi_query,
            )?
        } else {
            main_codegen.codegen_main_exit_code(root_fun, entry_argv_array)?
        };
        builder.build_return(Some(&exit_code))?;
        main_codegen.finish_function_explicit_frame_layout(root_fun.span)?;
    }

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module)
}

fn should_emit_reachable_fun_body(
    fun: &hir::FunDecl,
    unit_codegen: &crate::llvm::codegen::CompilationUnitCodegenCx<'_, '_>,
) -> bool {
    if fun.body.is_none() {
        return false;
    }

    if let Some(pass_view) = unit_codegen.materialized_pass_view() {
        if pass_view.callable_body_is_overridden(&fun.fqn) {
            return canonical_materialized_callable_body(fun, unit_codegen).is_some();
        }
        if pass_view.owner_of_callable(&fun.fqn).is_some()
            || unit_codegen
                .raw_non_generic_callable_candidate_body(fun, pass_view)
                .is_some()
        {
            return canonical_materialized_callable_body(fun, unit_codegen).is_some();
        }
    }

    true
}

fn canonical_materialized_callable_body<'a, 'ctx>(
    fun: &hir::FunDecl,
    unit_codegen: &'a crate::llvm::codegen::CompilationUnitCodegenCx<'a, 'ctx>,
) -> Option<&'a crate::mir::FunDecl> {
    let pass_view = unit_codegen.materialized_pass_view()?;
    if pass_view.callable_body_is_overridden(&fun.fqn)
        || pass_view.owner_of_callable(&fun.fqn).is_some()
    {
        return pass_view.callable(&fun.fqn);
    }
    unit_codegen.raw_non_generic_callable_candidate_body(fun, pass_view)
}

#[cfg(test)]
pub(crate) fn build_single_file_source_map(
    session: &Session,
    source: &SourceFile,
) -> (SourceMap, SourceId) {
    let input_sources = vec![source.clone()];
    frontend::build_source_map_with_extra_sources(session, &input_sources, 0)
}

fn entry_source(source_map: &SourceMap, entry_source_id: SourceId) -> &SourceFile {
    source_map
        .source(entry_source_id)
        .expect("entry source id should exist in source map")
}

#[derive(Clone, Copy)]
enum EntryMainArgShape {
    None,
    ArrayString,
}

#[derive(Clone, Copy)]
struct SelectedEntryMain<'a> {
    fun: &'a hir::FunDecl,
    arg_shape: EntryMainArgShape,
}

#[derive(Clone, Copy)]
struct SelectedRootCallable<'a> {
    fun: &'a hir::FunDecl,
    entry_main_arg_shape: Option<EntryMainArgShape>,
}

#[derive(Clone, Copy)]
enum RootCallableSelector<'a> {
    EntryMain {
        entry_main_fqn: Option<&'a str>,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    Callable {
        callable_fqn: &'a str,
    },
}

fn classify_entry_main_arg_shape(
    lowered: &hir::LoweredHir,
    fun: &hir::FunDecl,
) -> Option<EntryMainArgShape> {
    match fun.params.as_slice() {
        [] => Some(EntryMainArgShape::None),
        [param] => match lowered.types.kind(param.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Array"
                    && nominal.args.len() == 1
                    && matches!(
                        lowered.types.kind(nominal.args[0]),
                        TypeKind::Ref(RefTypeKind::String)
                    )
                    && nominal.eff.is_none() =>
            {
                Some(EntryMainArgShape::ArrayString)
            }
            _ => None,
        },
        _ => None,
    }
}

fn select_entry_main<'a>(
    lowered: &'a hir::LoweredHir,
    entry_main_fqn: Option<&str>,
) -> Result<SelectedEntryMain<'a>, LlvmEmitError> {
    let mut candidates = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .filter(|fun| {
            if let Some(entry_main_fqn) = entry_main_fqn {
                fun.fqn == entry_main_fqn
            } else {
                fun.name == "main"
            }
        })
        .filter(|fun| fun.body.is_some())
        .filter(|fun| {
            matches!(
                lowered.types.kind(fun.return_ty),
                TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Int)
            )
        })
        .filter_map(|fun| {
            classify_entry_main_arg_shape(lowered, fun)
                .map(|arg_shape| SelectedEntryMain { fun, arg_shape })
        })
        .collect::<Vec<_>>();

    match candidates.len() {
        0 => Err(LlvmEmitError::MissingEntryMain),
        1 => Ok(candidates.pop().expect("len checked above")),
        count => Err(LlvmEmitError::AmbiguousEntryMain {
            entry: entry_main_fqn.unwrap_or("main").to_string(),
            count,
        }),
    }
}

fn select_root_callable<'a>(
    lowered: &'a hir::LoweredHir,
    selector: RootCallableSelector<'_>,
) -> Result<SelectedRootCallable<'a>, LlvmEmitError> {
    match selector {
        RootCallableSelector::EntryMain { entry_main_fqn } => {
            let selected_main = select_entry_main(lowered, entry_main_fqn)?;
            Ok(SelectedRootCallable {
                fun: selected_main.fun,
                entry_main_arg_shape: Some(selected_main.arg_shape),
            })
        }
        RootCallableSelector::Callable { callable_fqn } => lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                hir::Item::Fun(fun) => Some(fun),
                _ => None,
            })
            .chain(lowered.member_funs.iter())
            .find(|fun| fun.fqn == callable_fqn && fun.body.is_some())
            .map(|fun| SelectedRootCallable {
                fun,
                entry_main_arg_shape: None,
            })
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!("显式历史 codegen helper 找不到 root callable `{callable_fqn}`"),
            }),
    }
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}
