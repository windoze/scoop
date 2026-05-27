//! Source-site typed contracts exported from HIR lowering.

use std::path::{Path, PathBuf};

use scoopc_ids::{CanonicalTextKey, SiteId};
use scoopc_source::SourceMapSpan;
use scoopc_span::Span;
use scoopc_types::{EffectRow, TypeId};

use crate::globals::GlobalStoragePolicy;

/// HIR facts keyed by source-level sites inside a body.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SourceSiteFacts {
    pub function_effects: Vec<FunctionEffectContract>,
    pub call_sites: Vec<CallSiteContract>,
    pub argument_bindings: Vec<ArgumentBindingContract>,
    pub assignments: Vec<AssignmentContract>,
    pub with_updates: Vec<WithUpdateContract>,
    pub perform_sites: Vec<PerformSiteContract>,
    pub handle_sites: Vec<HandleSiteContract>,
    pub continuation_resumes: Vec<ContinuationResumeContract>,
    pub pattern_bindings: Vec<PatternBindingContract>,
    pub top_level_init_roots: Vec<TopLevelInitRootContract>,
    pub extern_globals: Vec<ExternGlobalContract>,
}

impl SourceSiteFacts {
    /// Return whether no source-site contracts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.function_effects.is_empty()
            && self.call_sites.is_empty()
            && self.argument_bindings.is_empty()
            && self.assignments.is_empty()
            && self.with_updates.is_empty()
            && self.perform_sites.is_empty()
            && self.handle_sites.is_empty()
            && self.continuation_resumes.is_empty()
            && self.pattern_bindings.is_empty()
            && self.top_level_init_roots.is_empty()
            && self.extern_globals.is_empty()
    }

    /// Look up a call-site contract by source path and local span.
    pub fn call_site(&self, source_path: &Path, span: Span) -> Option<&CallSiteContract> {
        self.call_sites
            .iter()
            .find(|fact| fact.identity.matches_source(source_path, span))
    }

    /// Look up a perform contract by source path and local span.
    pub fn perform_site(&self, source_path: &Path, span: Span) -> Option<&PerformSiteContract> {
        self.perform_sites
            .iter()
            .find(|fact| fact.identity.matches_source(source_path, span))
    }

    /// Look up a handle contract by source path and local span.
    pub fn handle_site(&self, source_path: &Path, span: Span) -> Option<&HandleSiteContract> {
        self.handle_sites
            .iter()
            .find(|fact| fact.identity.matches_source(source_path, span))
    }

    /// Look up a continuation resume contract by source path and local span.
    pub fn continuation_resume(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<&ContinuationResumeContract> {
        self.continuation_resumes
            .iter()
            .find(|fact| fact.identity.matches_source(source_path, span))
    }

    /// Return whether a continuation resume site is known at this source location.
    pub fn has_continuation_resume(&self, source_path: &Path, span: Span) -> bool {
        self.continuation_resume(source_path, span).is_some()
    }

    /// Return whether a constructor call site is known at this source location.
    pub fn constructor_call(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<&ConstructorCallTarget> {
        match self.call_site(source_path, span).map(|fact| &fact.contract) {
            Some(CallSiteContractKind::Constructor(contract)) => Some(contract),
            _ => None,
        }
    }
}

/// Stable source-site identity scoped to a lowered body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SourceSiteIdentity {
    pub owner: CanonicalTextKey,
    pub site: SiteId,
    pub source_path: PathBuf,
    pub span: Span,
    pub source: Option<SourceMapSpan>,
}

impl SourceSiteIdentity {
    /// Create a source-site identity from its owner, local site id, and source span.
    pub fn new(owner: CanonicalTextKey, site: SiteId, source_path: PathBuf, span: Span) -> Self {
        Self {
            owner,
            site,
            source_path,
            span,
            source: None,
        }
    }

    /// Return whether this identity refers to the given source path and span.
    pub fn matches_source(&self, source_path: &Path, span: Span) -> bool {
        self.source_path == source_path && self.span == span
    }
}

/// Single callable's allowed effect row published by the HIR barrier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionEffectContract {
    pub fqn: String,
    pub source_path: PathBuf,
    pub span: Span,
    pub return_ty: TypeId,
    pub allowed_effects: EffectRow,
    pub effects_closed: bool,
}

/// Source-level category of a resolved call expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CallSiteKind {
    DirectTopLevel,
    MemberDirect,
    Extension,
    Constructor,
    Closure,
    FunValue,
    FunPtr,
    VirtualDispatch,
    InterfaceDispatch,
    Intrinsic,
    EffectOperation,
    ContinuationResume,
}

/// Typed contract for a resolved call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallSiteContract {
    pub identity: SourceSiteIdentity,
    pub kind: CallSiteKind,
    pub contract: CallSiteContractKind,
}

/// Detailed source-site contract for one call-like expression.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallSiteContractKind {
    DirectTopLevel(FunctionTarget),
    MemberDirect(MemberCallTarget),
    Extension {
        receiver_ty: TypeId,
        function: FunctionTarget,
    },
    Constructor(ConstructorCallTarget),
    Closure {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi: CallableAbi,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunValue {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi: CallableAbi,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunPtr {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi: CallableAbi,
        arg_binding: Option<CallArgBindingContract>,
    },
    Virtual(MemberCallTarget),
    Interface(MemberCallTarget),
    Intrinsic {
        kind: IntrinsicKind,
        function: FunctionTarget,
    },
    EffectOp(PerformSiteContract),
    ContinuationResume(ContinuationResumeContract),
}

/// Callable ABI family needed by later lowering without depending on HIR nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CallableAbi {
    ManagedOrdinary,
    NativeExtern,
    ManagedExtern,
    EffectBridge,
}

/// Compiler/runtime intrinsic family for a typed call-site fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IntrinsicKind {
    Reflection {
        name: String,
    },
    Platform {
        name: String,
    },
    Gc {
        name: String,
    },
    Runtime {
        name: String,
    },
    Compiler {
        name: String,
    },
    NamedTable {
        entry_name: String,
        uses_runtime_call: bool,
    },
}

/// Resolved function target identity and instantiation arguments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionTarget {
    pub fqn: String,
    pub decl_file: Option<PathBuf>,
    pub decl_span: Option<Span>,
    pub abi: CallableAbi,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
    pub arg_binding: Option<CallArgBindingContract>,
}

/// Member call target identity and receiver type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemberCallTarget {
    pub owner_fqn: String,
    pub member_name: String,
    pub member_fqn: String,
    pub receiver_ty: TypeId,
    pub function: FunctionTarget,
}

/// Constructor call target identity and argument mapping.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConstructorCallTarget {
    pub owner_fqn: String,
    pub ctor_span: Option<Span>,
    pub result_ty: TypeId,
    pub arg_mapping: Vec<Option<usize>>,
}

/// Canonical mapping from source arguments to callable parameter slots.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallArgBindingContract {
    pub params: Vec<CallArgParamContract>,
}

/// One parameter slot's source argument provenance.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallArgParamContract {
    Receiver,
    Explicit(CallArgElementContract),
    Default,
    Vararg(Vec<CallArgElementContract>),
}

/// Source argument element feeding a parameter or vararg slot.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallArgElementContract {
    pub arg_index: usize,
    pub spread: bool,
}

/// Canonical mapping from source argument position to callable parameter slot.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArgumentBindingContract {
    pub identity: SourceSiteIdentity,
    pub binding: CallArgBindingContract,
}

/// Typed contract for an assignment place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssignmentContract {
    pub identity: SourceSiteIdentity,
    pub span: Span,
    pub kind: AssignPlaceKind,
    pub place_ty: TypeId,
    pub value_ty: TypeId,
    pub mutable: bool,
    pub write_barrier: AssignWriteBarrierRequirement,
    pub unsafe_required: bool,
}

/// Assignment LHS family resolved during typecheck/HIR lowering.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssignPlaceKind {
    Local {
        symbol_id: u32,
        name: String,
        decl_span: Span,
    },
    TopLevel {
        symbol_id: u32,
        fqn: String,
    },
    Member {
        receiver_ty: TypeId,
        owner_fqn: Option<String>,
        member_fqn: String,
        member_name: String,
        member_span: Span,
        resolved: Option<MemberRef>,
    },
}

/// Member binding target used by assignment place metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemberRef {
    Value { symbol_id: u32, fqn: String },
    Fun { symbol_id: u32, fqn: String },
    ExtensionValue { symbol_id: u32, fqn: String },
    ExtensionFun { symbol_id: u32, fqn: String },
}

/// Write-barrier requirement attached to an assignment place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssignWriteBarrierRequirement {
    NotRequired,
    StorageSlot { slot_ty: TypeId },
}

/// Typed contract for aggregate copy/update syntax.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateContract {
    pub identity: SourceSiteIdentity,
    pub base_ty: TypeId,
    pub result_ty: TypeId,
    pub aggregates: Vec<WithUpdateAggregateContract>,
    pub updates: Vec<WithUpdateUpdateContract>,
}

/// One aggregate on a copy/update path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateAggregateContract {
    pub prefix: String,
    pub ty: TypeId,
    pub kind: WithUpdateAggregateContractKind,
}

/// Aggregate shape for a copy/update path segment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WithUpdateAggregateContractKind {
    Struct {
        fqn: String,
        fields: Vec<WithUpdateAggregateFieldContract>,
    },
    Tuple {
        elements: Vec<TypeId>,
    },
    Enum {
        info: WithUpdateResolvedEnum,
    },
}

/// Field metadata for a struct aggregate on a copy/update path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateAggregateFieldContract {
    pub name: String,
    pub ty: TypeId,
}

/// One user-visible update in aggregate copy/update syntax.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateUpdateContract {
    pub path: String,
    pub target_ty: TypeId,
    pub value_ty: TypeId,
    pub segments: Vec<WithUpdatePathSegmentContract>,
}

/// One segment in an aggregate copy/update path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdatePathSegmentContract {
    pub aggregate_prefix: String,
    pub aggregate_ty: TypeId,
    pub field_ty: TypeId,
    pub kind: WithUpdatePathSegmentKind,
}

/// Copy/update path segment family.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WithUpdatePathSegmentKind {
    StructField {
        owner_fqn: String,
        field: String,
    },
    TupleElement {
        index: usize,
    },
    EnumVariantField {
        enum_fqn: String,
        variant: String,
        field: String,
    },
}

/// Resolved enum shape for aggregate copy/update syntax.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateResolvedEnum {
    pub enum_fqn: String,
    pub variants: Vec<WithUpdateResolvedEnumVariant>,
}

/// Resolved enum variant shape for aggregate copy/update syntax.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateResolvedEnumVariant {
    pub name: String,
    pub fields: Vec<WithUpdateResolvedEnumField>,
}

/// Resolved enum field shape for aggregate copy/update syntax.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateResolvedEnumField {
    pub name: String,
    pub ty: TypeId,
}

/// Structured typed payload for `perform` / `handle` sites.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PayloadTypeContract {
    pub ty: Option<TypeId>,
    pub components: Vec<TypeId>,
}

/// Single `perform` site typed contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PerformSiteContract {
    pub identity: SourceSiteIdentity,
    pub effect_ty: TypeId,
    pub op_fqn: String,
    pub result_ty: TypeId,
    pub payload: PayloadTypeContract,
    pub arg_mapping: Vec<usize>,
}

/// Stable typed HIR kind for a `handle` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HandleArmContractKind {
    NonResuming,
    EscapeContinuation,
}

/// Single `handle` arm typed contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HandleArmSiteContract {
    pub handled_effect_ty: TypeId,
    pub op_fqn: String,
    pub payload: PayloadTypeContract,
    pub body_ty: TypeId,
    pub kind: HandleArmContractKind,
}

/// Single `handle { ... } on { ... }` site typed contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HandleSiteContract {
    pub identity: SourceSiteIdentity,
    pub result_ty: TypeId,
    pub body_result_ty: TypeId,
    pub arm_contracts: Vec<HandleArmSiteContract>,
    pub finally_result_ty: Option<TypeId>,
}

/// MIR lowering should not rediscover the continuation receiver from callee syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ContinuationResumeReceiverRoute {
    CallArg { index: usize },
    MemberReceiver,
}

/// Typed contract for a continuation resume site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContinuationResumeContract {
    pub identity: SourceSiteIdentity,
    pub receiver_route: ContinuationResumeReceiverRoute,
    pub payload_arg_indices: Vec<usize>,
    pub receiver_ty: TypeId,
    pub resume_ty: TypeId,
    pub answer_ty: TypeId,
    pub return_ty: TypeId,
    pub out_effects: EffectRow,
    pub runtime_error_effect_ty: Option<TypeId>,
}

impl ContinuationResumeContract {
    /// Return whether this resume can suspend outside the current handler.
    pub fn resumes_outward(&self) -> bool {
        !self.out_effects.is_pure()
    }
}

/// Precise type assigned to a source pattern binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternBindingContract {
    pub identity: SourceSiteIdentity,
    pub binding_name: String,
    pub binding_ty: TypeId,
}

/// Typed HIR handoff root for top-level initialization/storage ordering.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TopLevelInitRootContract {
    pub fqn: String,
    pub source_path: PathBuf,
    pub span: Span,
    pub kind: TopLevelInitRootKind,
    pub ty: Option<TypeId>,
    pub initializer_ty: Option<TypeId>,
    pub has_initializer: bool,
    pub dependencies: Vec<TopLevelInitDependency>,
}

/// Top-level initialization root family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TopLevelInitRootKind {
    RuntimeImmutableVal,
    RuntimeMutableVar { storage: GlobalStoragePolicy },
    ObjectSingleton,
}

/// Initialization dependency edge for a top-level root.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TopLevelInitDependency {
    pub fqn: String,
    pub kind: TopLevelInitDependencyKind,
}

/// Initialization dependency family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TopLevelInitDependencyKind {
    TopLevelValue,
    ObjectSingleton,
}

/// Typed HIR handoff contract for an `@Extern` top-level variable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternGlobalContract {
    pub fqn: String,
    pub source_path: PathBuf,
    pub span: Span,
    pub ty: TypeId,
    pub mutable: bool,
    pub symbol: String,
    pub linkage: ExternGlobalLinkage,
    pub storage: GlobalStoragePolicy,
    pub initializer_absent: bool,
    pub unsafe_required: bool,
}

/// Extern global linkage family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExternGlobalLinkage {
    External,
}
