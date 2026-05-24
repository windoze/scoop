//! Program-wide ABI materialization for the late-lowered effect IR.
//!
//! The `ProgramAbiMaterializer` walks the published `LateLoweredProgram` and
//! materializes every artifact the LLVM backend needs to emit code:
//! state-machine step / frame layouts, callable + continuation shells,
//! surface resume bindings, carrier entry shells, boundary operand and
//! payload contracts, dynamic-invoke layouts, and handle dispatch tables.
//! The submodules below own one phase each — see each module's `//!` comment
//! for its specific responsibility.
//!
//! `mod.rs` carries the shared types (the `ProgramAbiMaterializer` struct,
//! the `BoundaryOperand*` map aliases, the `MaterializedDynamicCallSite`
//! key type) plus a handful of cross-module utilities used by every phase
//! (`dummy_span`, `frontend_error`, render helpers, the
//! `validate_program_layout_inventory` precondition check).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, StructType};

use crate::effect_facts::{CallSiteKind, CallTargetMode, ContinuationSchemaId, StepSchemaId};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    BoundaryId, BoundarySiteKind, ContinuationObjectId, LateLoweredBodyVersionKey,
    LateLoweredBoundaryLowering, LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallable,
    LateLoweredCompletionPayloadBinding, LateLoweredCompletionPayloadSource,
    LateLoweredContinuationMethodReachability, LateLoweredContinuationObject,
    LateLoweredFrameSlotKind, LateLoweredHandlePendingCompletion,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredOperandSource,
    LateLoweredOperandValueSource, LateLoweredPublishedRuntimeEntry, LateLoweredResumeInterface,
    LateLoweredResumePayloadBinding, LateLoweredSourceStatementClassificationKind,
    LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepType,
    LateLoweredSurfaceResumeContract, LateLoweredSurfaceResumeDispatchInventoryEntry,
    LateLoweredSurfaceResumeDispatchPublication, LateLoweredSurfaceResumeWrapperCaseProjection,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    LateLoweredSurfaceResumeWrapperCompleteProjection, LateLoweredSurfaceResumeWrapperProjection,
    ResumeInterfaceId, StateId, SystemSlotKind,
};
use crate::effect_lowered::mir_source::{BasicBlockId, InstanceKey, SiteId};
use crate::llvm::LlvmEmitError;
use crate::stable_id::canonical_record;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use scoopc_lir_facts::{
    LirCallTargetMode, LirCallableContract, LirCallableFacts, LirDynamicInvokeCarrierKind,
    LirDynamicInvokeContract, LirFacts,
};

use super::super::types::IntTy;
use super::super::{AbiMangler, CallableCarrierKind, LlvmFunctionDeclarationSurface, MainCodegen};
use super::stable_naming;
use super::types::{
    AbiValue, CallBoundaryOperandLayout, CallableCarrierTargetLayout, CallableEntryLayout,
    CallableLayout, ClassInstanceFieldLayout, ClassInstanceLayout, ClosureCarrierLayout,
    CompletionPayloadBindingLayout, ContinuationFieldKind, ContinuationFieldLayout,
    ContinuationObjectLayout, ContinuationSurfaceResumeBinding,
    ContinuationSurfaceResumeDispatchLayout, ContinuationSurfaceResumeDispatchTarget,
    ContinuationSurfaceResumeHandleBinderRoute, ContinuationSurfaceResumeLayout,
    ContinuationSurfaceResumeMethodLookup, ContinuationSurfaceResumeOwnerTrampolineLayout,
    DispatchReceiverLayout, DynamicInvokeCarrierLayout, DynamicInvokeLayout, FrameFieldKind,
    FrameFieldLayout, FrameLayout, HandleArmLayout, HandleContinuationBinderLayout,
    HandleDispatchLayout, HandlePayloadBinderLayout, HandlePendingPayloadTransportLayout,
    LocalRuntimeErrorContract, LocalRuntimeErrorTerminalAction, PerformBoundaryOperandLayout,
    PlainCallableEntryLayout, PlainCallableLayout, ProgramAbiQuery, PublishedRuntimeEntryLayout,
    ResumeBoundaryOperandLayout, ResumeInterfaceLayout, ResumeMethodLayout,
    ResumePayloadBindingLayout, SourceAbiFieldLayout, SourceAbiLayout, SourceAbiLayoutKind,
    StepCaseLayout, StepLayout, StepVariantLayout,
};

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// P6-T02：把 P5 late-lowered contract 显式物化成 LLVM type/layout 查询面。
    pub(crate) fn materialize_program_abi(
        &mut self,
        program: &'a LateLoweredProgram,
        lir_facts: &'a LirFacts,
        source_types: &'a TypeStore,
    ) -> Result<ProgramAbiQuery<'ctx>, LlvmEmitError> {
        ProgramAbiMaterializer::new(self, program, lir_facts, source_types)?.materialize()
    }
}

type BoundaryOperandKey = (StepSchemaId, SiteId);
type CallBoundaryOperandLayouts = BTreeMap<BoundaryOperandKey, CallBoundaryOperandLayout>;
type PerformBoundaryOperandLayouts = BTreeMap<BoundaryOperandKey, PerformBoundaryOperandLayout>;
type ResumeBoundaryOperandLayouts = BTreeMap<BoundaryOperandKey, ResumeBoundaryOperandLayout>;
type ResumePayloadBindingBoundaryKey = (StepSchemaId, BoundaryId);
type ResumePayloadBindingStateKey = (StepSchemaId, StateId);
type ResumePayloadBindingLayouts =
    BTreeMap<ResumePayloadBindingBoundaryKey, ResumePayloadBindingLayout>;
type ResumePayloadBindingLayoutsByState =
    BTreeMap<ResumePayloadBindingStateKey, ResumePayloadBindingLayout>;
type CompletionPayloadBindingKey = (StepSchemaId, StateId);
type CompletionPayloadBindingLayouts<'ctx> =
    BTreeMap<CompletionPayloadBindingKey, CompletionPayloadBindingLayout<'ctx>>;
type BoundaryOperandLayoutSets = (
    CallBoundaryOperandLayouts,
    PerformBoundaryOperandLayouts,
    ResumeBoundaryOperandLayouts,
);

fn validate_program_layout_inventory(program: &LateLoweredProgram) -> Result<(), LlvmEmitError> {
    let mut step_types_by_schema = BTreeMap::new();
    for step_type in program.step_types() {
        if step_types_by_schema
            .insert(step_type.step_schema(), step_type)
            .is_some()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 遇到重复 StepSchemaId {}",
                step_type.step_schema().as_u32()
            )));
        }
    }

    let mut callables_by_step_schema = BTreeMap::new();
    for callable in program.callables() {
        let Some(step_schema) = callable.body_step_schema() else {
            continue;
        };
        if callables_by_step_schema
            .insert(step_schema, callable)
            .is_some()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 遇到重复 callable step schema {}（callable={})",
                step_schema.as_u32(),
                callable.root_fqn()
            )));
        }
    }

    let mut continuation_objects_by_id = BTreeMap::new();
    for object in program.continuation_objects() {
        if continuation_objects_by_id
            .insert(object.object_id(), object)
            .is_some()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 遇到重复 continuation object {}",
                object.object_id().as_u32()
            )));
        }
    }

    let mut resume_packings_by_id = BTreeMap::new();
    for interface in program.resume_packings() {
        if resume_packings_by_id
            .insert(interface.interface_id(), interface)
            .is_some()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 遇到重复 resume packing {}",
                interface.interface_id().as_u32()
            )));
        }
    }

    Ok(())
}

struct ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    codegen: &'cg mut MainCodegen<'a, 'ctx>,
    program: &'a LateLoweredProgram,
    lir_facts: &'a LirFacts,
    source_types: &'a TypeStore,
    source_value_layouts: BTreeMap<TypeId, SourceAbiLayout<'ctx>>,
}

// ----- cross-module utilities -----

fn dummy_span() -> crate::span::Span {
    crate::span::Span::new(0, 0)
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn layout_type_is_any(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Any))
}

fn render_resume_packing_ids(interface_ids: &[ResumeInterfaceId]) -> String {
    format!(
        "[{}]",
        interface_ids
            .iter()
            .map(|interface_id| interface_id.as_u32().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_case_tags(tags: &BTreeSet<crate::effect_facts::CaseTag>) -> String {
    if tags.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        tags.iter()
            .map(|case_tag| format!("c{}", case_tag.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

// ----- submodules -----

mod abi;
mod boundary;
mod callable;
mod carrier;
mod dynamic_invoke;
mod handle_dispatch;
mod lookup;
mod orchestrator;
mod payload;
mod state_machine;
mod surface_resume;

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
mod tests;
