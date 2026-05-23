use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::effect_facts::{
    BodyEffectFacts, CallSiteEffectFacts, CallTargetMode, CaseTag, ClassCtorSiteEffectFacts,
    ConcreteOpKey, EffectFamilyKey, HandleSiteEffectFacts, ImplPlan, MaterializedEffectFacts,
    PerformSiteEffectFacts, ResumeSiteEffectFacts, SiteEffectFacts, StepCaseFact, StepSchema,
    StepSchemaId,
};
use crate::mir::{
    BasicBlockId, Body, CallArg, CallKind, LocalId, MaterializedMirPassView, MemberAccessMetadata,
    MemberTarget, Operand, PatternBindingStep, PerformArg, ResumeMetadata, Rvalue, SiteId,
    StatementKind, StoredContinuationRoutePublication, StoredContinuationValueRoute,
    TerminatorKind,
};
use crate::ty::{NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::EffectLoweringError;
use super::ir::{
    BoundaryId, BoundarySiteKind, ContinuationObjectId, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallBoundaryLowering,
    LateLoweredCallBoundaryOperandContract, LateLoweredClassCtorBoundaryLowering,
    LateLoweredCompleteStepDispatch, LateLoweredCompletionPayloadBinding,
    LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationCapture, LateLoweredContinuationContract, LateLoweredContinuationMethod,
    LateLoweredContinuationObject, LateLoweredContinuationResumeBody,
    LateLoweredContinuationSurfaceResume, LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema,
    LateLoweredFrameSlotKind, LateLoweredHandleArmDispatch, LateLoweredHandleBoundaryCaseRouting,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandleBoundaryLowering,
    LateLoweredHandleBoundaryRouting, LateLoweredHandleContinuationBinder,
    LateLoweredHandleDispatchCarrierContract, LateLoweredHandleDispatchContract,
    LateLoweredHandlePayloadBinder, LateLoweredHandlePendingCompletion,
    LateLoweredHandlePendingCompletionOrigin, LateLoweredHandlePendingPayloadTransport,
    LateLoweredHandleStateRegion, LateLoweredHandleStateRegionEntry,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredOneShotPolicy, LateLoweredOperandSource,
    LateLoweredPerformBoundaryLowering, LateLoweredPerformBoundaryOperandContract,
    LateLoweredPublishedRuntimeEntry, LateLoweredResumeBoundaryLowering,
    LateLoweredResumeBoundaryOperandContract, LateLoweredResumeInterface, LateLoweredResumeMethod,
    LateLoweredResumePayloadBinding, LateLoweredRuntimeErrorBoundaryLowering,
    LateLoweredSourceStatementClassification, LateLoweredSourceStatementClassificationKind,
    LateLoweredState, LateLoweredStateGraph, LateLoweredStateRole, LateLoweredStateSlice,
    LateLoweredStateTerminator, LateLoweredStepCase, LateLoweredStepCaseEmission,
    LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan, LateLoweredStepType,
    ResumeInterfaceId, StateId,
};
use super::ir::{
    LateLoweredBodyVersionKey, LateLoweredBoundarySource, LateLoweredContinuationRoute,
    LateLoweredSurfaceResumeDispatchPublication,
};

pub(crate) type NominalDirectSupertypeIndex = HashMap<String, Vec<String>>;

pub(crate) struct StepMaterialization {
    pub(crate) step_types: Vec<LateLoweredStepType>,
    pub(crate) resume_packings: Vec<LateLoweredResumeInterface>,
    pub(crate) resume_packing_ids_by_step: BTreeMap<StepSchemaId, Vec<ResumeInterfaceId>>,
    pub(crate) resume_packing_ids_by_group:
        BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
}

pub(crate) struct BoundaryMaterializationInputs<'a> {
    pub(crate) root_fqn: &'a str,
    pub(crate) owner_version_key: &'a LateLoweredBodyVersionKey,
    pub(crate) body: &'a Body,
    pub(crate) body_facts: &'a BodyEffectFacts,
    pub(crate) step_type: &'a LateLoweredStepType,
    pub(crate) state_graph: &'a LateLoweredStateGraph,
    pub(crate) frame_schema: &'a LateLoweredFrameSchema,
    pub(crate) boundary_map: &'a LateLoweredBoundaryMap,
    pub(crate) continuation_object: ContinuationObjectId,
    pub(crate) step_types: &'a [LateLoweredStepType],
    pub(crate) types: &'a TypeStore,
    pub(crate) nominal_direct_supertypes: &'a NominalDirectSupertypeIndex,
    pub(crate) cross_callable_continuation_provenance:
        Option<&'a CrossCallableContinuationProvenance>,
}

#[derive(Debug, Clone)]
pub(crate) struct ContinuationRouteOwnerPlan {
    owner_version_key: LateLoweredBodyVersionKey,
    continuation_object: ContinuationObjectId,
}

impl ContinuationRouteOwnerPlan {
    pub(crate) fn new(
        owner_version_key: LateLoweredBodyVersionKey,
        continuation_object: ContinuationObjectId,
    ) -> Self {
        Self {
            owner_version_key,
            continuation_object,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CrossCallableContinuationProvenance {
    member_routes_by_callee: HashMap<String, Vec<CrossCallableContinuationMemberRoute>>,
    global_member_routes:
        HashMap<GlobalContinuationMemberKey, Vec<CrossCallableGlobalContinuationMemberRoute>>,
}

impl CrossCallableContinuationProvenance {
    fn routes_for_callee(&self, callee_fqn: &str) -> &[CrossCallableContinuationMemberRoute] {
        self.member_routes_by_callee
            .get(callee_fqn)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn routes_for_global_member(
        &self,
        receiver_fqn: &str,
        member: &ContinuationMemberIdentityKey,
    ) -> &[CrossCallableGlobalContinuationMemberRoute] {
        let key = GlobalContinuationMemberKey {
            receiver_fqn: receiver_fqn.to_string(),
            member: member.clone(),
        };
        self.global_member_routes
            .get(&key)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CrossCallableContinuationMemberRoute {
    param_index: usize,
    member: ContinuationMemberIdentityKey,
    path: Vec<PatternBindingStep>,
    route: LateLoweredContinuationRoute,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GlobalContinuationMemberKey {
    receiver_fqn: String,
    member: ContinuationMemberIdentityKey,
}

#[derive(Debug, Clone)]
pub(crate) struct CrossCallableGlobalContinuationMemberRoute {
    path: Vec<PatternBindingStep>,
    route: LateLoweredContinuationRoute,
}

pub(crate) struct ContinuationObjectMaterializationInputs<'a> {
    pub(crate) continuation_object_id: ContinuationObjectId,
    pub(crate) owner_version_key: LateLoweredBodyVersionKey,
    pub(crate) step_schema_id: StepSchemaId,
    pub(crate) step_schema: &'a StepSchema,
    pub(crate) implemented_packings: &'a [ResumeInterfaceId],
    pub(crate) resume_packing_ids_by_group:
        &'a BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
    pub(crate) captures: Vec<LateLoweredContinuationCapture>,
    pub(crate) effect_facts: &'a MaterializedEffectFacts,
}

pub(crate) struct CallBoundaryDispatchMaterialization {
    dispatch: LateLoweredStepDispatchPlan,
    continuation_compositions: Vec<LateLoweredCallBoundaryContinuationComposition>,
    consumed_runtime_error_case: Option<PendingConsumedRuntimeErrorCase>,
}

pub(crate) struct BoundaryMaterialization {
    pub(crate) state_graph: LateLoweredStateGraph,
    pub(crate) boundary_map: LateLoweredBoundaryMap,
}

pub(crate) struct PendingConsumedRuntimeErrorCase {
    input_case_tag: crate::effect_facts::CaseTag,
    input_concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

pub(crate) struct LocalRuntimeErrorStateTarget {
    boundary_id: BoundaryId,
    owner_state: StateId,
    target_state: StateId,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

pub(crate) struct CallBoundaryDispatchInputs<'a> {
    root_fqn: &'a str,
    boundary_id: BoundaryId,
    input_step: &'a LateLoweredStepType,
    output_step: &'a LateLoweredStepType,
    outward_case_tags: &'a [crate::effect_facts::CaseTag],
    continuation_object: ContinuationObjectId,
    target_state: StateId,
    result_local: Option<LocalId>,
    result_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
    types: &'a TypeStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ContinuationMemberIdentityKey {
    Value(String),
    Fun(String),
    ExtensionValue(String),
    ExtensionFun(String),
    Unresolved { name: String, receiver_ty: TypeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ContinuationMemberKey {
    receiver_local: LocalId,
    member: ContinuationMemberIdentityKey,
}

impl ContinuationMemberKey {
    fn from_metadata(receiver_local: LocalId, member: &MemberAccessMetadata) -> Self {
        let member = match member.resolved.as_ref() {
            Some(MemberTarget::Value { fqn }) => ContinuationMemberIdentityKey::Value(fqn.clone()),
            Some(MemberTarget::Fun { fqn }) => ContinuationMemberIdentityKey::Fun(fqn.clone()),
            Some(MemberTarget::ExtensionValue { fqn }) => {
                ContinuationMemberIdentityKey::ExtensionValue(fqn.clone())
            }
            Some(MemberTarget::ExtensionFun { fqn }) => {
                ContinuationMemberIdentityKey::ExtensionFun(fqn.clone())
            }
            None => ContinuationMemberIdentityKey::Unresolved {
                name: member.name.clone(),
                receiver_ty: member.receiver_ty,
            },
        };
        Self {
            receiver_local,
            member,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalContinuationOrigin {
    Seed(LateLoweredContinuationRoute),
    Copy(LocalId),
    AggregateElement {
        source: LocalId,
        path: Vec<PatternBindingStep>,
    },
    MemberRead(ContinuationMemberKey),
    PatternExtract {
        subject: LocalId,
        path: Vec<PatternBindingStep>,
    },
    PatternMemberRead {
        key: ContinuationMemberKey,
        path: Vec<PatternBindingStep>,
    },
}

#[derive(Default)]
pub(crate) struct PublishedContinuationProvenance {
    local_origins: HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    member_store_routes: HashMap<ContinuationMemberKey, Vec<PublishedMemberStoreRoute>>,
}

mod classification;
mod contract_op;
mod contract_step;
pub mod dispatch_plan;
mod main;
mod provenance;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use {
    classification::*, contract_op::*, contract_step::*, dispatch_plan::*, main::*, provenance::*,
};
