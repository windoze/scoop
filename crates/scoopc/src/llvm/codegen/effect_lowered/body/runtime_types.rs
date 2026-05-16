//! Runtime data types used to thread state through the per-callable emitter
//! during state-machine lowering.
//!
//! These small structs and enums describe transient runtime decisions
//! (consume/dispatch/goto/outward actions, pending payload state, completion
//! mode, runtime-error origin) that the lowering logic builds up while
//! walking the published state graph. They are private to this module — none
//! of them appear in the public ABI; they are scratchpads for the LLVM
//! emitter only.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefactorHandleCompletionMode {
    ContinueToExit,
    ReturnFromFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefactorCallableReturnMode {
    Step,
    EffectOutcome,
    Plain { declared_return_cg: CgTy },
}

impl RefactorHandleCompletionMode {
    pub(super) fn pending_completion(self) -> LateLoweredHandlePendingCompletion {
        match self {
            Self::ContinueToExit => LateLoweredHandlePendingCompletion::ContinueToExit,
            Self::ReturnFromFunction => LateLoweredHandlePendingCompletion::ReturnFromFunction,
        }
    }
}

#[derive(Clone)]
pub(super) struct RefactorHandleConsumeArmRuntime {
    pub(super) site_id: SiteId,
    pub(super) arm_ordinal: u32,
    pub(super) arm_state: StateId,
    pub(super) payload_binders: Vec<RefactorHandlePayloadBinderLayout>,
    pub(super) continuation_binder: Option<RefactorHandleContinuationBinderLayout>,
}

#[derive(Clone)]
pub(super) struct RefactorHandleBoundaryDispatchCandidate {
    pub(super) dispatch_identity: u64,
    pub(super) action: RefactorHandleBoundaryRuntimeAction,
}

#[derive(Clone, Copy)]
pub(super) struct RefactorHandlePendingPayloadRuntime {
    pub(super) completion: LateLoweredHandlePendingCompletion,
    pub(super) payload_tuple_ty: TypeId,
    pub(super) frame_field_index: u32,
}

#[derive(Clone)]
pub(super) struct RefactorHandlePendingCompletionRuntime {
    pub(super) site_id: SiteId,
    pub(super) completion: LateLoweredHandlePendingCompletion,
    pub(super) completion_tag_value: u32,
    pub(super) completion_tag_field_index: u32,
    pub(super) finally_state: StateId,
    pub(super) payload_transport: Option<RefactorHandlePendingPayloadRuntime>,
}

#[derive(Clone)]
pub(super) struct RefactorLocalRuntimeErrorRuntime {
    pub(super) site_id: SiteId,
    pub(super) input_case_tag: CaseTag,
    pub(super) payload_tuple_ty: TypeId,
    pub(super) target_state: StateId,
    pub(super) runtime_symbol: String,
    pub(super) runtime_param_count: usize,
}

#[derive(Clone)]
pub(super) enum RefactorHandleBoundaryRuntimeAction {
    ConsumeToArm(RefactorHandleConsumeArmRuntime),
    PendingCompletion(RefactorHandlePendingCompletionRuntime),
    EmitOutward,
}

#[derive(Clone)]
pub(super) enum RefactorHandleGotoRuntimeAction {
    RestoreSavedCtxAndGoto {
        clear_slots: bool,
        site_id: SiteId,
        target: StateId,
    },
    BeginCompletion(RefactorHandlePendingCompletionRuntime),
    FinishFinally(RefactorHandleFinallyRuntime),
}

#[derive(Clone, Copy)]
pub(super) struct RefactorHandleOutwardCompletionRuntime {
    pub(super) boundary_id: BoundaryId,
    pub(super) completion_tag_value: u32,
    pub(super) case_tag: CaseTag,
    pub(super) payload_tuple_ty: TypeId,
    pub(super) resume_state: StateId,
    pub(super) payload_transport: Option<RefactorHandlePendingPayloadRuntime>,
}

#[derive(Clone)]
pub(super) struct RefactorHandleFinallyRuntime {
    pub(super) site_id: SiteId,
    pub(super) completion_tag_field_index: u32,
    pub(super) exit_state: StateId,
    pub(super) continue_to_exit_tag: u32,
    pub(super) return_from_function_tag: u32,
    pub(super) return_payload_source: Option<LateLoweredCompletionPayloadSource>,
    pub(super) propagate_outward: Vec<RefactorHandleOutwardCompletionRuntime>,
}

#[derive(Clone, Copy)]
pub(super) struct RefactorResumeUnwindOrigin<'a> {
    pub(super) suspend_state: StateId,
    pub(super) cleanup_state: StateId,
    pub(super) resume_state: StateId,
    pub(super) boundary_ids: &'a [BoundaryId],
}

pub(super) enum RefactorClassCtorBoundarySource<'a> {
    ClassCtor {
        span: crate::span::Span,
        ctor: &'a mir::ClassCtorCallMetadata,
        args: &'a [mir::CallArg],
    },
    ObjectProperty {
        span: crate::span::Span,
        fqn: &'a str,
    },
    TopLevelRef {
        span: crate::span::Span,
        fqn: &'a str,
    },
}

pub(super) struct TaskTransportResumeCandidate<'a, 'ctx> {
    pub(super) callable: &'a LateLoweredCallable,
    pub(super) adapter: FunctionValue<'ctx>,
    pub(super) type_desc_i8: PointerValue<'ctx>,
    pub(super) dispatch_plan: LateLoweredStepDispatchPlan,
}
