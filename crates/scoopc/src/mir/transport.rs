use crate::span::Span;
use crate::ty::TypeId;

use super::LocalId;

/// Backend-agnostic transport classification for a MIR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirTransportKind {
    Scalar,
    Reference,
    Tuple,
    Struct,
    EnumPayload,
    ClosureEnv,
    CaptureBox,
    ArrayElement,
    EffectPayload,
    FunctionValue,
    Unknown,
}

/// Why an aggregate value must be boxed before crossing a transport boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBoxingReason {
    AnyErasure,
    RefErasure,
    EffectPayload,
    ClosureCapture,
    ArrayElement,
    FunctionValueAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirBoxingIntent {
    pub source_ty: TypeId,
    pub target_ty: Option<TypeId>,
    pub reason: MirBoxingReason,
}

/// Copy/drop/trace obligations that later layout/codegen must honor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirTransportRequirements {
    pub trace: bool,
    pub copy: bool,
    pub drop: bool,
}

impl MirTransportRequirements {
    pub const fn plain_value() -> Self {
        Self {
            trace: false,
            copy: true,
            drop: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueTransportMetadata {
    pub source_ty: TypeId,
    pub kind: MirTransportKind,
    pub requirements: MirTransportRequirements,
    pub boxing: Option<MirBoxingIntent>,
}

impl ValueTransportMetadata {
    pub fn plain(source_ty: TypeId, kind: MirTransportKind) -> Self {
        Self {
            source_ty,
            kind,
            requirements: MirTransportRequirements::plain_value(),
            boxing: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateTransportKind {
    Tuple,
    Struct,
    EnumPayload,
    ClosureEnv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTransportField {
    pub index: usize,
    pub name: Option<String>,
    pub ty: TypeId,
    pub transport: ValueTransportMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTransportMetadata {
    pub aggregate_ty: TypeId,
    pub kind: AggregateTransportKind,
    pub fields: Vec<AggregateTransportField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBoxTransportMetadata {
    pub box_ty: TypeId,
    pub value: ValueTransportMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureCaptureTransportMetadata {
    pub name: String,
    pub decl_span: Span,
    pub mutable: bool,
    pub source_local: LocalId,
    pub transport: ValueTransportMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureEnvTransportMetadata {
    pub env_ty: TypeId,
    pub captures: Vec<ClosureCaptureTransportMetadata>,
}

impl ClosureEnvTransportMetadata {
    pub fn empty(env_ty: TypeId) -> Self {
        Self {
            env_ty,
            captures: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayTransportOperation {
    BuilderNew,
    BuilderPush,
    BuilderBuildArray,
    BuilderBuildMutableArray,
    Get,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayElementTransportMetadata {
    pub operation: ArrayTransportOperation,
    pub array_ty: TypeId,
    pub element_ty: TypeId,
    pub mutable: bool,
    pub element: ValueTransportMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcIntrinsicOperation {
    Pin,
    Unpin,
    HandleNew,
    HandleGet,
    HandleDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcRootLifetime {
    PinnedUntilUnpin,
    EndsPinnedRoot,
    StableHandleUntilDrop,
    BorrowedFromStableHandle,
    EndsStableHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcIntrinsicPairing {
    PinMustPairUnpin,
    UnpinMatchesPin,
    HandleNewMustPairDrop,
    HandleGetRequiresLiveHandle,
    HandleDropMatchesHandleNew,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcIntrinsicTransportMetadata {
    pub callee_fqn: String,
    pub operation: GcIntrinsicOperation,
    pub root_lifetime: GcRootLifetime,
    pub pairing: GcIntrinsicPairing,
    pub unsafe_required: bool,
    pub subject_ty: TypeId,
    pub token_ty: Option<TypeId>,
    pub subject: ValueTransportMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirCallableAbiKind {
    Plain,
    EffectStep,
    DeferredToEffectFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirCallableImplPlan {
    NoOutward,
    SingleCase,
    CanonicalFull,
    DeferredToEffectFacts,
}

/// MIR-side ABI handoff marker for call-like values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallAbiHandoffMetadata {
    pub callable_abi_kind: MirCallableAbiKind,
    pub resolved_outward_cases: Vec<String>,
    pub impl_plan: MirCallableImplPlan,
    pub adapter_required: bool,
}

impl CallAbiHandoffMetadata {
    pub fn deferred_to_effect_facts() -> Self {
        Self {
            callable_abi_kind: MirCallableAbiKind::DeferredToEffectFacts,
            resolved_outward_cases: Vec::new(),
            impl_plan: MirCallableImplPlan::DeferredToEffectFacts,
            adapter_required: false,
        }
    }

    pub fn plain_no_outward() -> Self {
        Self {
            callable_abi_kind: MirCallableAbiKind::Plain,
            resolved_outward_cases: Vec::new(),
            impl_plan: MirCallableImplPlan::NoOutward,
            adapter_required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTransportMetadata {
    pub result: ValueTransportMetadata,
    pub aggregate_return: Option<ValueTransportMetadata>,
    pub array: Option<ArrayElementTransportMetadata>,
    pub gc: Option<GcIntrinsicTransportMetadata>,
    pub abi: CallAbiHandoffMetadata,
}

impl CallTransportMetadata {
    pub fn plain_no_outward(result_ty: TypeId, kind: MirTransportKind) -> Self {
        Self {
            result: ValueTransportMetadata::plain(result_ty, kind),
            aggregate_return: None,
            array: None,
            gc: None,
            abi: CallAbiHandoffMetadata::plain_no_outward(),
        }
    }
}
