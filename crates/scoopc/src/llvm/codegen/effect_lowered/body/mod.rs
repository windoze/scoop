//! LLVM effect-lowered body codegen（P6-T03）。
//!
//! This module lowers the P5 late-lowered state graph directly.  Generic MIR
//! lowering is reused only for effect-neutral source slices; every boundary,
//! resume payload binding, completion payload, and state transition comes from
//! the published late-lowered / ABI query contract.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use inkwell::AddressSpace;
use inkwell::AtomicOrdering;
use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, IntValue,
    PointerValue, StructValue,
};

use crate::effect_facts::{CaseTag, StepSchemaId};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    BoundaryId, BoundarySiteKind, FrameSlotId, LateLoweredBoundary, LateLoweredBoundaryLowering,
    LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallBoundaryLowering,
    LateLoweredCallable, LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationResumeBody, LateLoweredFrameSlotKind,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandlePendingCompletion,
    LateLoweredHandlePendingCompletionOrigin, LateLoweredHandleStateRegion,
    LateLoweredOperandSource, LateLoweredOperandValueSource, LateLoweredPlainBodySlice,
    LateLoweredPlainCallable, LateLoweredResumePayloadBinding, LateLoweredSourceBody,
    LateLoweredSourceCallable, LateLoweredSourceStatementClassificationKind, LateLoweredState,
    LateLoweredStateRole, LateLoweredStateTerminator, LateLoweredStepCaseForwarding,
    LateLoweredStepDispatchPlan, LateLoweredSurfaceResumeDispatchPublication,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource, ResumeInterfaceId, StateId,
    SystemSlotKind,
};
use crate::effect_lowered::mir_source::{self as mir, LocalId, SiteId};
use crate::llvm::LlvmEmitError;
use crate::stable_id::{canonical_record, canonical_type_text};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::effect_outcome::{EffectOutcomeTag, ValueTransportParts};
use super::super::mir_body::{MirLocalSlot, collect_mir_local_uses};
use super::super::types::{CgTy, CgValue};
use super::super::{
    CallableCarrierKind, EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR, MainCodegen, NativeCallableAbi,
    TypeDescriptorSpec,
};
use super::stable_naming;
use super::types::{
    CallTargetQuery, CallableEntryLayout, CallableLayout, ContinuationSurfaceResumeDispatchTarget,
    ContinuationSurfaceResumeLayout, DynamicInvokeCarrierLayout, DynamicInvokeLayout, FrameLayout,
    HandleContinuationBinderLayout, HandlePayloadBinderLayout, LocalRuntimeErrorTerminalAction,
    PlainCallableEntryLayout, PlainCallableLayout, ProgramAbiQuery, SourceAbiLayout,
    SourceAbiLayoutKind, StepCaseLayout, StepLayout, StepVariantLayout,
};
use super::value::ValuePrimitives;

const STEP_TAG_COMPLETE: u64 = 0;
const MAIN_UNHANDLED_EXIT_CODE: u64 = 3;
const CONT_FIELD_RESUMED: u32 = 1;
const CONT_FIELD_RESUME_STATE: u32 = 2;
const CONT_FIELD_CAPTURED_EFFECT_CTX: u32 = 3;
const CONT_FIELD_STATE_REF: u32 = 4;
const CONT_FIELD_STEP_FN: u32 = 5;
const CONT_FIELD_RESUME_WORD: u32 = 6;
const CONT_FIELD_RESUME_GC_REF: u32 = 7;
const CONT_FIELD_CAPTURED_CALLEE_SUSPEND_STATE: u32 = 8;

// ----- post-impl module-level helper functions -----
//
// Each of these functions is a small pure helper used across multiple
// submodules to inspect late-lowered IR shapes. They live here (not on
// `CallableEmitter`) because they take only borrowed inputs and
// don't need access to per-callable codegen state.

fn callable_source<'a>(
    callable: &'a LateLoweredCallable,
    context: &str,
) -> Result<&'a LateLoweredSourceCallable, LlvmEmitError> {
    callable.source_callable().ok_or_else(|| {
        frontend_error(format!(
            "{context} callable `{}` 缺少 LIR-owned source callable body contract",
            callable.root_fqn()
        ))
    })
}

fn callable_source_body<'a>(
    callable: &'a LateLoweredCallable,
    context: &str,
) -> Result<(&'a LateLoweredSourceCallable, &'a LateLoweredSourceBody), LlvmEmitError> {
    let source = callable_source(callable, context)?;
    let body = source.body.as_ref().ok_or_else(|| {
        frontend_error(format!(
            "{context} callable `{}` 缺少 LIR-owned source body contract",
            callable.root_fqn()
        ))
    })?;
    Ok((source, body))
}

fn resume_packing_method_is_reachable(
    program: &LateLoweredProgram,
    interface_id: ResumeInterfaceId,
    case_tag: CaseTag,
) -> bool {
    program.continuation_objects().iter().any(|object| {
        object.implemented_packings().contains(&interface_id)
            && object.methods().iter().any(|method| {
                method.packing_interface_id() == interface_id
                    && method.case_tag() == case_tag
                    && matches!(
                        method.body(),
                        LateLoweredContinuationResumeBody::ResumeCapturedState { .. }
                    )
            })
    })
}

fn boundary_site(boundary: &LateLoweredBoundary, expected: &str) -> Result<SiteId, LlvmEmitError> {
    match boundary.source() {
        LateLoweredBoundarySource::Site { site_id, .. } => Ok(site_id),
        other => Err(frontend_error(format!(
            "{expected} boundary bd{} 绑定到非 site source {other:?}",
            boundary.boundary_id().as_u32()
        ))),
    }
}

fn boundary_source_consumption(
    boundary: &LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.source_consumption()),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) => None,
        LateLoweredBoundaryLowering::Handle(_) => None,
    }
}

fn handle_finally_state(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> Option<StateId> {
    let mut states = contract.state_regions().iter().filter_map(|entry| {
        matches!(entry.region(), LateLoweredHandleStateRegion::Finally).then_some(entry.state_id())
    });
    let first = states.next()?;
    if states.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn handle_finally_return_payload_source(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> Result<Option<LateLoweredCompletionPayloadSource>, LlvmEmitError> {
    if !contract.needs_completion_state() {
        return Ok(None);
    }
    if let Some(source) = contract.body_completion_payload_source().cloned() {
        return Ok(Some(source));
    }
    let mut published = None;
    for arm in contract.handled_arms() {
        let candidate = arm.completion_payload_source();
        match &published {
            Some(existing) if same_completion_payload_source_ignoring_span(existing, candidate) => {
            }
            Some(existing) => {
                return Err(frontend_error(format!(
                    "HandleDispatch finally ReturnFromFunction completion payload source 歧义：body/previous={existing:?} arm={candidate:?}"
                )));
            }
            None => published = Some(candidate.clone()),
        }
    }
    Ok(published)
}

fn boundary_complete_result_local(boundary: &LateLoweredBoundary) -> Option<LocalId> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::Resume(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::Perform(_)
        | LateLoweredBoundaryLowering::RuntimeError(_)
        | LateLoweredBoundaryLowering::Handle(_) => None,
    }
}

fn completion_payload_source_is_local(
    source: &LateLoweredCompletionPayloadSource,
    local: LocalId,
) -> bool {
    matches!(
        source,
        LateLoweredCompletionPayloadSource::Operand(operand)
            if matches!(operand.value(), LateLoweredOperandValueSource::Local(source_local) if *source_local == local)
    )
}

fn completion_payload_local_pair(
    binding_source: &LateLoweredCompletionPayloadSource,
    return_source: &LateLoweredCompletionPayloadSource,
) -> Option<(LocalId, LocalId)> {
    let LateLoweredCompletionPayloadSource::Operand(binding_operand) = binding_source else {
        return None;
    };
    let LateLoweredCompletionPayloadSource::Operand(return_operand) = return_source else {
        return None;
    };
    if binding_operand.source_ty() != return_operand.source_ty() {
        return None;
    }
    let LateLoweredOperandValueSource::Local(binding_local) = binding_operand.value() else {
        return None;
    };
    let LateLoweredOperandValueSource::Local(return_local) = return_operand.value() else {
        return None;
    };
    Some((*binding_local, *return_local))
}

fn validate_callable_entry_layout(layout: &CallableLayout<'_>) -> Result<(), LlvmEmitError> {
    let direct = layout.direct_entry();
    let dynamic = layout.dynamic_entry();
    if direct.invoke_args_tuple_ty() != dynamic.invoke_args_tuple_ty()
        || direct.param_count() != dynamic.param_count()
        || direct.args_abi().is_elided() != dynamic.args_abi().is_elided()
        || direct.return_step_schema() != dynamic.return_step_schema()
        || direct.return_step_schema() != layout.step_schema()
    {
        return Err(frontend_error(format!(
            "callable `{}` entry ABI contract 漂移：direct=(args=t{}, params={}, elided={}, return=s{}) dynamic=(args=t{}, params={}, elided={}, return=s{}) layout_step=s{}",
            layout.root_fqn(),
            direct.invoke_args_tuple_ty().as_u32(),
            direct.param_count(),
            direct.args_abi().is_elided(),
            direct.return_step_schema().as_u32(),
            dynamic.invoke_args_tuple_ty().as_u32(),
            dynamic.param_count(),
            dynamic.args_abi().is_elided(),
            dynamic.return_step_schema().as_u32(),
            layout.step_schema().as_u32(),
        )));
    }
    Ok(())
}

fn validate_plain_callable_layout(
    callable: &LateLoweredCallable,
    layout: &PlainCallableLayout<'_>,
) -> Result<(), LlvmEmitError> {
    let plain = callable.plain_abi().ok_or_else(|| {
        frontend_error(format!(
            "plain callable `{}` 缺少 plain ABI handoff",
            callable.root_fqn()
        ))
    })?;
    let entry = layout.direct_entry();
    if layout.root_fqn() != callable.root_fqn()
        || entry.param_tys() != plain.param_tys()
        || entry.return_ty() != plain.return_ty()
    {
        return Err(frontend_error(format!(
            "plain callable `{}` ABI contract 漂移：layout_root=`{}` return=t{} params={:?} handoff_function=t{} handoff_return=t{} handoff_params={:?}",
            callable.root_fqn(),
            layout.root_fqn(),
            entry.return_ty().as_u32(),
            entry.param_tys(),
            plain.function_ty().as_u32(),
            plain.return_ty().as_u32(),
            plain.param_tys(),
        )));
    }
    Ok(())
}

fn validate_plain_body_slices(
    root_fqn: &str,
    plain: &LateLoweredPlainCallable,
    body: &LateLoweredSourceBody,
) -> Result<BTreeMap<mir::BasicBlockId, LateLoweredPlainBodySlice>, LlvmEmitError> {
    if plain.body_slices().len() != body.blocks.len() {
        return Err(frontend_error(format!(
            "plain callable `{root_fqn}` 的 body_slices 数量({}) 与 MIR block 数量({}) 不一致",
            plain.body_slices().len(),
            body.blocks.len(),
        )));
    }
    let mut slices = BTreeMap::new();
    for slice in plain.body_slices() {
        let block = body
            .blocks
            .get(slice.block_id().as_u32() as usize)
            .ok_or_else(|| {
                frontend_error(format!(
                    "plain callable `{root_fqn}` 的 source slice 指向缺失 bb{}",
                    slice.block_id().as_u32()
                ))
            })?;
        if slice.start_statement_index() != 0
            || slice.end_statement_index() as usize != block.stmts.len()
            || !slice.includes_terminator()
        {
            return Err(frontend_error(format!(
                "plain callable `{root_fqn}` 的 bb{} source slice 不是完整 ordinary block：slice=[{}..{}) includes_terminator={} stmt_count={}",
                slice.block_id().as_u32(),
                slice.start_statement_index(),
                slice.end_statement_index(),
                slice.includes_terminator(),
                block.stmts.len(),
            )));
        }
        if slices.insert(slice.block_id(), *slice).is_some() {
            return Err(frontend_error(format!(
                "plain callable `{root_fqn}` 重复发布 bb{} source slice",
                slice.block_id().as_u32()
            )));
        }
    }
    Ok(slices)
}

fn frame_slot_local(kind: crate::effect_lowered::ir::LateLoweredFrameSlotKind) -> Option<LocalId> {
    match kind {
        crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(local)
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(local)
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::JoinValue { local, .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder { local, .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::BoundaryResult { local, .. } => {
            Some(local)
        }
        crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleSavedEffectCtx { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleArmEffectCtx { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(_) => None,
    }
}

fn same_completion_payload_source_ignoring_span(
    left: &LateLoweredCompletionPayloadSource,
    right: &LateLoweredCompletionPayloadSource,
) -> bool {
    match (left, right) {
        (
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: left_ty,
            },
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredCompletionPayloadSource::Operand(left),
            LateLoweredCompletionPayloadSource::Operand(right),
        ) => left.source_ty() == right.source_ty() && left.value() == right.value(),
        _ => false,
    }
}

fn source_layout_component_count(layout: &SourceAbiLayout<'_>) -> usize {
    if layout.abi().is_elided() {
        return 0;
    }
    match layout.kind() {
        SourceAbiLayoutKind::Scalar => 1,
        SourceAbiLayoutKind::Tuple => layout
            .fields()
            .iter()
            .map(|field| field.source_index() as usize + 1)
            .max()
            .unwrap_or(0),
    }
}

fn handle_dispatch_region_implies_runtime_nesting(region: LateLoweredHandleStateRegion) -> bool {
    // `Exit` 只是前一个 handle 的收尾落点，不代表当前 dispatch 仍在它的动态作用域内。
    !matches!(
        region,
        LateLoweredHandleStateRegion::OutsideHandle | LateLoweredHandleStateRegion::Exit
    )
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

// ----- submodules -----

mod call_invoke;
mod class_ctor;
mod composed_call;
mod continuation;
mod effect_outcome;
mod emit_entry;
mod emitter;
mod frame;
mod gc_alloc;
mod handle_boundary;
mod handle_completion;
mod lower_source;
mod main_carrier;
mod main_entry;
mod main_resume;
mod native_callable;
mod payload;
mod runtime_error;
mod runtime_types;
mod states;
mod step_case;
mod symbol_naming;
mod verification;
mod wrapper;

// Re-exports so sibling submodules can refer to these names via `use super::*;`.
use emitter::{CallableEmitter, ComposedBoundaryDispatchContext};
use runtime_types::*;
use symbol_naming::*;
