//! Backend-neutral contracts published next to the LIR body.

use scoop_project_model::StableConeKey;
use scoopc_ids::{BodyVersionKey, SiteId, StableLirCallableKey};
use scoopc_types::TypeId;

macro_rules! id_key {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn as_u32(self) -> u32 {
                self.0
            }
        }
    };
}

id_key!(/// Stable StepSchema identity as published by LIR facts.
    LirStepSchemaKey);
id_key!(/// Stable continuation schema identity as published by LIR facts.
    LirContinuationSchemaKey);
id_key!(/// Stable state identity scoped to one LIR control body.
    LirStateKey);
id_key!(/// Stable boundary identity scoped to one LIR control body.
    LirBoundaryKey);
id_key!(/// Stable frame-slot identity scoped to one LIR frame schema.
    LirFrameSlotKey);
id_key!(/// Stable resume-packing identity scoped to one LIR program.
    LirResumePackingKey);
id_key!(/// Stable continuation-object identity scoped to one LIR program.
    LirContinuationObjectKey);
id_key!(/// Stable case identity scoped to one StepSchema.
    LirCaseKey);
id_key!(/// Stable block identity as observed through LIR source slices.
    LirBodyBlockKey);
id_key!(/// Stable local identity scoped to one source body.
    LirLocalKey);
id_key!(/// Stable per-cone init routine identity scoped to one LIR fact product.
    LirConeInitRoutineKey);

/// Stable global/init root identity published by LIR facts.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirGlobalRootKey(String);

impl LirGlobalRootKey {
    pub fn new(fqn: impl Into<String>) -> Self {
        Self(fqn.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Backend-neutral global/init root family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LirGlobalRootKind {
    TopLevelImmutableVal,
    TopLevelMutableVar,
    ObjectSingleton,
    ExternGlobal,
}

impl LirGlobalRootKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::TopLevelImmutableVal => "top_level_immutable_val",
            Self::TopLevelMutableVar => "top_level_mutable_var",
            Self::ObjectSingleton => "object_singleton",
            Self::ExternGlobal => "extern_global",
        }
    }
}

/// Backend-neutral storage policy for mutable or extern global storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LirGlobalStoragePolicy {
    Global,
    ThreadLocal,
}

impl LirGlobalStoragePolicy {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::ThreadLocal => "thread_local",
        }
    }
}

/// Stable dependency categories for global initialization roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LirGlobalDependencyKind {
    TopLevelValue,
    ObjectSingleton,
}

impl LirGlobalDependencyKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::TopLevelValue => "top_level_value",
            Self::ObjectSingleton => "object_singleton",
        }
    }
}

/// Backend-neutral source/body family for a published initializer contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LirInitializerBodyKind {
    TopLevelImmutableVal,
    TopLevelMutableVar,
    ObjectSingleton,
}

impl LirInitializerBodyKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::TopLevelImmutableVal => "top_level_immutable_val",
            Self::TopLevelMutableVar => "top_level_mutable_var",
            Self::ObjectSingleton => "object_singleton",
        }
    }
}

/// Source/body handoff for a root initializer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirInitializerBodyFacts {
    pub root: LirGlobalRootKey,
    pub kind: LirInitializerBodyKind,
    pub source_path: String,
    pub source_span_start: usize,
    pub source_span_end: usize,
    pub body_item_count: usize,
}

/// Dependency edge from one global/init root to another published root.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirGlobalRootDependency {
    pub target: LirGlobalRootKey,
    pub kind: LirGlobalDependencyKind,
}

/// Extern global declaration contract needed by backend physicalization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirExternGlobalFacts {
    pub symbol: String,
    pub linkage: LirExternGlobalLinkage,
    pub mutable: bool,
    pub initializer_absent: bool,
    pub unsafe_required: bool,
}

/// Backend-neutral linkage for an extern global declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LirExternGlobalLinkage {
    External,
}

impl LirExternGlobalLinkage {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::External => "external",
        }
    }
}

/// Complete backend-neutral contract for one global/init root.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirGlobalRootFacts {
    pub root: LirGlobalRootKey,
    pub kind: LirGlobalRootKind,
    pub cone: StableConeKey,
    pub source_cone_order: u32,
    pub ty: Option<TypeId>,
    pub storage: Option<LirGlobalStoragePolicy>,
    pub has_initializer: bool,
    pub dependencies: Vec<LirGlobalRootDependency>,
    pub source_path: Option<String>,
    pub extern_global: Option<LirExternGlobalFacts>,
    pub initializer_body: Option<LirInitializerBodyFacts>,
}

/// Object singleton once-initialization contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirObjectOnceFacts {
    pub root: LirGlobalRootKey,
    pub has_initializer: bool,
}

/// Top-level eager initialization contract executed before user entry bodies.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirTopLevelEagerInitFacts {
    pub root: LirGlobalRootKey,
    pub storage: Option<LirGlobalStoragePolicy>,
    pub has_initializer: bool,
}

/// Per-cone eager init routine contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirConeInitRoutineFacts {
    pub routine: LirConeInitRoutineKey,
    pub cone: StableConeKey,
    pub source_cone_order: u32,
    pub roots: Vec<LirGlobalRootKey>,
}

/// Final system-entry ordering for per-cone init routines.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirFinalEntryInitOrderFacts {
    pub routines: Vec<LirConeInitRoutineKey>,
}

/// Global initialization and storage contract group owned by LIR facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirGlobalInitFacts {
    pub roots: std::collections::BTreeMap<LirGlobalRootKey, LirGlobalRootFacts>,
    pub object_once: std::collections::BTreeMap<LirGlobalRootKey, LirObjectOnceFacts>,
    pub top_level_eager_inits:
        std::collections::BTreeMap<LirGlobalRootKey, LirTopLevelEagerInitFacts>,
    pub cone_init_routines:
        std::collections::BTreeMap<LirConeInitRoutineKey, LirConeInitRoutineFacts>,
    pub final_entry_order: LirFinalEntryInitOrderFacts,
}

impl LirGlobalInitFacts {
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
            && self.object_once.is_empty()
            && self.top_level_eager_inits.is_empty()
            && self.cone_init_routines.is_empty()
            && self.final_entry_order.routines.is_empty()
    }
}

/// Class field layout facts needed before backend-private physicalization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassFieldFacts {
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
}

/// Class instance layout facts published without carrying the HIR class-init table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassLayoutFacts {
    pub fqn: String,
    pub layout_key: String,
    pub super_class_fqn: Option<String>,
    pub fields: Vec<LirClassFieldFacts>,
}

/// Backend-neutral enum representation family.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirEnumReprFacts {
    TaggedUnion,
    ValueOnly { underlying_ty_fqn: Option<String> },
}

impl LirEnumReprFacts {
    pub const fn stable_name(&self) -> &'static str {
        match self {
            Self::TaggedUnion => "tagged_union",
            Self::ValueOnly { .. } => "value_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirEnumVariantFieldFacts {
    pub name: String,
    pub ty: Option<TypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirEnumVariantFacts {
    pub name: String,
    pub tag: u64,
    pub fields: Vec<LirEnumVariantFieldFacts>,
}

/// Enum variant/repr facts published without the HIR enum-layout side table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirEnumLayoutFacts {
    pub fqn: String,
    pub repr: LirEnumReprFacts,
    pub variants: Vec<LirEnumVariantFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassVtableSlotFacts {
    pub slot: u32,
    pub name: String,
    pub params_len: u32,
    pub has_receiver: bool,
    pub impl_member_fqn: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirInterfaceMethodSlotFacts {
    pub slot: u32,
    pub name: String,
    pub member_fqn: String,
    pub params_len: u32,
    pub has_receiver: bool,
    pub has_body: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirInterfaceLayoutFacts {
    pub fqn: String,
    pub interface_id: u64,
    pub super_interfaces: Vec<String>,
    pub method_slots: Vec<LirInterfaceMethodSlotFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassItableEntryFacts {
    pub interface_fqn: String,
    pub interface_id: u64,
    pub interface_type_name: String,
    pub interface_type_id: u64,
    pub runtime_match_type_names: Vec<String>,
    pub runtime_match_type_ids: Vec<u64>,
    pub method_impl_fqns: Vec<String>,
    pub method_receiver_type_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassItableFacts {
    pub class_fqn: String,
    pub entries: Vec<LirClassItableEntryFacts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallableSymbolKind {
    ManagedOrdinary,
    NativeExtern,
    ManagedExtern,
    EffectBridge,
}

impl LirCallableSymbolKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::ManagedOrdinary => "managed_ordinary",
            Self::NativeExtern => "native_extern",
            Self::ManagedExtern => "managed_extern",
            Self::EffectBridge => "effect_bridge",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirNativeCallableSignatureFacts {
    pub symbol: String,
    pub calling_convention: String,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirExternCallableSignatureFacts {
    pub symbol: String,
    pub abi: String,
    pub calling_convention: Option<String>,
    pub lib: Option<String>,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
}

/// Source callable signature published for body-less/runtime/helper call targets.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirSourceCallableSignatureFacts {
    pub root_fqn: String,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
}

/// Stable identity for a class constructor init body contract.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirClassCtorInitKey(String);

impl LirClassCtorInitKey {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn for_ctor(class_fqn: &str, ctor_span: Option<(usize, usize)>) -> Self {
        let suffix = ctor_span
            .map(|(start, end)| format!("{start}..{end}"))
            .unwrap_or_else(|| "default".to_string());
        Self(format!("class_ctor_init:{class_fqn}@{suffix}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Source-level constructor family selected before backend lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirClassCtorKind {
    Primary,
    Secondary,
}

impl LirClassCtorKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}

/// Constructor delegation shape fixed by the LIR ctor-init contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirClassCtorDelegationKind {
    This,
    Super,
}

impl LirClassCtorDelegationKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::This => "this",
            Self::Super => "super",
        }
    }
}

/// One executable step in a class constructor init body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirClassCtorInitStepKind {
    PropertyParamAssignment,
    PropertyInitializer,
    InitBlock,
    SecondaryBody,
}

impl LirClassCtorInitStepKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::PropertyParamAssignment => "property_param_assignment",
            Self::PropertyInitializer => "property_initializer",
            Self::InitBlock => "init_block",
            Self::SecondaryBody => "secondary_body",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorParamFacts {
    pub name: String,
    pub ty: TypeId,
    pub has_default: bool,
    pub is_property: bool,
    pub property_field_fqn: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorSuperCallFacts {
    pub target: LirClassCtorInitKey,
    pub class_fqn: String,
    pub arg_count: usize,
    pub source_span_start: Option<usize>,
    pub source_span_end: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorDelegationFacts {
    pub kind: LirClassCtorDelegationKind,
    pub target: LirClassCtorInitKey,
    pub class_fqn: String,
    pub arg_count: usize,
    pub source_span_start: usize,
    pub source_span_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorInitStepFacts {
    pub kind: LirClassCtorInitStepKind,
    pub field_fqn: Option<String>,
    pub source_span_start: usize,
    pub source_span_end: usize,
}

/// Backend-neutral class constructor init body ownership contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorInitFacts {
    pub key: LirClassCtorInitKey,
    pub class_fqn: String,
    pub source_path: String,
    pub ctor_kind: LirClassCtorKind,
    pub ctor_span_start: Option<usize>,
    pub ctor_span_end: Option<usize>,
    pub params: Vec<LirClassCtorParamFacts>,
    pub implicit_super: Option<LirClassCtorSuperCallFacts>,
    pub delegation: Option<LirClassCtorDelegationFacts>,
    pub steps: Vec<LirClassCtorInitStepFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallableSymbolFacts {
    pub callable: StableLirCallableKey,
    pub root_fqn: String,
    pub stable_instance_key: String,
    pub exported_symbol: Option<String>,
    pub kind: LirCallableSymbolKind,
    pub abi_kind: LirCallableAbiKind,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub native: Option<LirNativeCallableSignatureFacts>,
    pub extern_: Option<LirExternCallableSignatureFacts>,
}

/// Physical ABI/layout contracts that LLVM may map to backend-private LLVM types.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirPhysicalLayoutFacts {
    pub classes: std::collections::BTreeMap<String, LirClassLayoutFacts>,
    pub enums: std::collections::BTreeMap<String, LirEnumLayoutFacts>,
    pub class_vtables: std::collections::BTreeMap<String, Vec<LirClassVtableSlotFacts>>,
    pub interfaces: std::collections::BTreeMap<String, LirInterfaceLayoutFacts>,
    pub class_itables: std::collections::BTreeMap<String, LirClassItableFacts>,
    pub callable_symbols: std::collections::BTreeMap<StableLirCallableKey, LirCallableSymbolFacts>,
}

impl LirPhysicalLayoutFacts {
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
            && self.enums.is_empty()
            && self.class_vtables.is_empty()
            && self.interfaces.is_empty()
            && self.class_itables.is_empty()
            && self.callable_symbols.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirTypeContextOwner {
    LirStageBaseContext,
}

impl LirTypeContextOwner {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::LirStageBaseContext => "lir_stage_base_context",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirTypeContextBridgeMode {
    Identical,
    ExplicitDisplayNameRemap,
}

impl LirTypeContextBridgeMode {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::ExplicitDisplayNameRemap => "explicit_display_name_remap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirTypeStableWireFormatDecision {
    Implemented,
    Deferred,
}

impl LirTypeStableWireFormatDecision {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirTypeStableWireFormatFacts {
    pub decision: LirTypeStableWireFormatDecision,
    pub owner: String,
    pub reason: String,
    pub non_blocking_reason: String,
}

/// Type context bridge facts for LIR/backend consumers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirTypeContextFacts {
    pub owner: LirTypeContextOwner,
    pub primary_fingerprint: String,
    pub materialized_fingerprint: String,
    pub effect_facts_fingerprint: String,
    pub bridge_mode: LirTypeContextBridgeMode,
    pub remapped_type_count: usize,
    pub stable_wire_format: LirTypeStableWireFormatFacts,
}

impl Default for LirTypeContextFacts {
    fn default() -> Self {
        Self {
            owner: LirTypeContextOwner::LirStageBaseContext,
            primary_fingerprint: String::new(),
            materialized_fingerprint: String::new(),
            effect_facts_fingerprint: String::new(),
            bridge_mode: LirTypeContextBridgeMode::Identical,
            remapped_type_count: 0,
            stable_wire_format: LirTypeStableWireFormatFacts {
                decision: LirTypeStableWireFormatDecision::Deferred,
                owner: String::new(),
                reason: String::new(),
                non_blocking_reason: String::new(),
            },
        }
    }
}

/// Stable source slice retained by a plain callable or control-body state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirSourceSliceKey {
    pub block_id: LirBodyBlockKey,
    pub start_statement_index: u32,
    pub end_statement_index: u32,
    pub includes_terminator: bool,
}

/// Stable dynamic-invoke identity scoped by owner callable and source site.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirDynamicInvokeKey {
    pub owner_callable: StableLirCallableKey,
    pub site_id: SiteId,
}

/// Stable dispatch identity scoped by owner callable and source site.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirDispatchKey {
    pub owner_callable: StableLirCallableKey,
    pub site_id: SiteId,
}

/// Body-version identity and semantic flags selected before LIR lowering.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirBodyVersionFacts {
    pub key: BodyVersionKey,
    pub impl_plan: String,
    pub needs_reentry: bool,
    pub allowed_effect_terms: Vec<TypeId>,
}

/// Callable ABI family published by LIR facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallableKind {
    Plain,
    EffectStep,
}

/// Source-level callable family used by backend entry selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallableSourceKind {
    TopLevel,
    MemberOrSynthetic,
}

/// Backend-neutral call-site source kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallSiteKind {
    Direct,
    Closure,
    FunValue,
    FunPtr,
    Virtual,
    Interface,
}

/// Target-resolution mode for a LIR call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallTargetMode {
    KnownInstance,
    CandidateSet,
    DynamicFallback,
}

/// Callable ABI selected for a call-site target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallableAbiKind {
    Plain,
    EffectStep,
}

/// Stable revision for the current LIR optimization family contract.
pub const LIR_OPT_PIPELINE_REVISION: u64 = 1;

/// Named pass family owned by LIR optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirOptPassKind {
    LocalStateMachineElimination,
    HigherOrderWrapperInlineDevirt,
    WrapperStateFolding,
    DynamicInvokeEntryRewrite,
    DeadStateSlotCleanup,
    ResumePackingPruning,
    PostOptVerifier,
}

impl LirOptPassKind {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::LocalStateMachineElimination => "local-state-machine-elimination",
            Self::HigherOrderWrapperInlineDevirt => "higher-order-wrapper-inline-devirt",
            Self::WrapperStateFolding => "wrapper-state-folding",
            Self::DynamicInvokeEntryRewrite => "dynamic-invoke-entry-rewrite",
            Self::DeadStateSlotCleanup => "dead-state-slot-cleanup",
            Self::ResumePackingPruning => "resume-packing-pruning",
            Self::PostOptVerifier => "post-opt-verifier",
        }
    }
}

/// Whether a named LIR opt pass ran or was intentionally disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirOptPassStatus {
    Applied,
    NoOp,
    Skipped,
}

/// Stable metadata for one LIR opt pass invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirOptPassFacts {
    pub kind: LirOptPassKind,
    pub status: LirOptPassStatus,
    pub changed: bool,
}

impl LirOptPassFacts {
    pub const fn new(kind: LirOptPassKind, status: LirOptPassStatus, changed: bool) -> Self {
        Self {
            kind,
            status,
            changed,
        }
    }
}

/// Pipeline metadata binding LIR facts to the post-opt LIR body revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirOptPipelineFacts {
    pub revision: u64,
    pub preserve_published_resume_shells: bool,
    pub passes: Vec<LirOptPassFacts>,
}

impl LirOptPipelineFacts {
    pub fn new(
        revision: u64,
        preserve_published_resume_shells: bool,
        passes: Vec<LirOptPassFacts>,
    ) -> Self {
        Self {
            revision,
            preserve_published_resume_shells,
            passes,
        }
    }

    pub fn empty(revision: u64) -> Self {
        Self {
            revision,
            preserve_published_resume_shells: false,
            passes: Vec::new(),
        }
    }
}

/// Precision of a published call-site effect/control contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirEffectPrecision {
    Precise,
    Widened,
    SignatureFallback,
}

/// Structured call-site contract after replacing raw MIR target keys with stable LIR keys.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallSiteContract {
    pub kind: LirCallSiteKind,
    pub target_mode: LirCallTargetMode,
    pub target_callables: Vec<StableLirCallableKey>,
    pub callee_abi_kind: LirCallableAbiKind,
    pub invoke_args_tuple_ty: TypeId,
    pub callee_step_schema: Option<LirStepSchemaKey>,
    pub resolved_cases: Vec<LirCaseKey>,
    pub precision: LirEffectPrecision,
}

/// Plain callable source slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirPlainBodySliceFacts {
    pub source_slice: LirSourceSliceKey,
}

/// Plain callable call site with its source-slice identity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirPlainCallSiteFacts {
    pub site_id: SiteId,
    pub source_slice: LirSourceSliceKey,
    pub statement_index: u32,
    pub contract: LirCallSiteContract,
    pub dynamic_invoke: Option<LirDynamicInvokeKey>,
    pub dispatch: Option<LirDispatchKey>,
}

/// Plain callable ordinary ABI and body-source contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirPlainCallableFacts {
    pub function_ty: TypeId,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub body_slices: Vec<LirPlainBodySliceFacts>,
    pub call_sites: Vec<LirPlainCallSiteFacts>,
    pub local_effect_control: Option<LirControlBodyFacts>,
}

/// Canonical dynamic callable surface for an effect-step callable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallableDynamicInvokeEntryFacts {
    pub invoke_args_tuple_ty: TypeId,
    pub step_schema: LirStepSchemaKey,
    pub entry_state: LirStateKey,
    pub complete_state: LirStateKey,
}

/// Effect-step callable ABI and control-body contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirEffectStepCallableFacts {
    pub param_tys: Vec<TypeId>,
    pub closure_carrier_arg_tys: Vec<TypeId>,
    pub step_schema: LirStepSchemaKey,
    pub dynamic_invoke_entry: LirCallableDynamicInvokeEntryFacts,
    pub control_body: LirControlBodyFacts,
}

/// Callable-specific ABI contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCallableContract {
    Plain(Box<LirPlainCallableFacts>),
    EffectStep(Box<LirEffectStepCallableFacts>),
}

impl LirCallableContract {
    pub fn kind(&self) -> LirCallableKind {
        match self {
            Self::Plain(_) => LirCallableKind::Plain,
            Self::EffectStep(_) => LirCallableKind::EffectStep,
        }
    }

    pub fn has_control_body(&self) -> bool {
        match self {
            Self::Plain(plain) => plain.local_effect_control.is_some(),
            Self::EffectStep(_) => true,
        }
    }
}

/// Complete callable inventory entry and ABI/query contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallableFacts {
    pub root_fqn: String,
    pub stable_instance_key: String,
    pub source_kind: LirCallableSourceKind,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub body_version: LirBodyVersionFacts,
    pub resolved_outward_cases: Vec<LirCaseKey>,
    pub contract: LirCallableContract,
}

impl LirCallableFacts {
    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn kind(&self) -> LirCallableKind {
        self.contract.kind()
    }

    pub fn has_control_body(&self) -> bool {
        self.contract.has_control_body()
    }

    pub fn is_top_level_source_callable(&self) -> bool {
        self.source_kind == LirCallableSourceKind::TopLevel
    }
}

/// Query keys published for a callable state graph.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirStateGraphFacts {
    pub entry_state: LirStateKey,
    pub complete_state: LirStateKey,
    pub cleanup_state: Option<LirStateKey>,
    pub drop_state: Option<LirStateKey>,
    pub states: Vec<LirStateKey>,
}

/// Query keys and payload bindings published for a frame schema.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirFrameSchemaFacts {
    pub slots: Vec<LirFrameSlotFacts>,
    pub resume_payload_bindings: Vec<LirResumePayloadBindingFacts>,
    pub completion_payload_bindings: Vec<LirCompletionPayloadBindingFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirFrameSlotFacts {
    pub slot_id: LirFrameSlotKey,
    pub ty: TypeId,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirResumePayloadBindingFacts {
    pub boundary_id: LirBoundaryKey,
    pub resume_state: LirStateKey,
    pub consumer_local: LirLocalKey,
    pub consumer_frame_slot: Option<LirFrameSlotKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCompletionPayloadBindingFacts {
    pub return_state: LirStateKey,
    pub complete_state: LirStateKey,
    pub payload_frame_slot: Option<LirFrameSlotKey>,
}

/// Boundary-map query keys and attached call/dynamic/dispatch contracts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirBoundaryMapFacts {
    pub boundaries: Vec<LirBoundaryFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirBoundaryFacts {
    pub boundary_id: LirBoundaryKey,
    pub source_kind: String,
    pub site_id: Option<SiteId>,
    pub owner_state: LirStateKey,
    pub resume_state: LirStateKey,
    pub lowering_kind: Option<String>,
    pub dynamic_invoke: Option<LirDynamicInvokeKey>,
    pub dispatch: Option<LirDispatchKey>,
}

/// Resume-state query keys for a control body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirResumeStateMapFacts {
    pub entries: Vec<LirResumeStateFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirResumeStateFacts {
    pub boundary_id: LirBoundaryKey,
    pub state_id: LirStateKey,
}

/// Shared control-body contract used by effect-step callables and plain local control.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirControlBodyFacts {
    pub step_schema: LirStepSchemaKey,
    pub state_graph: LirStateGraphFacts,
    pub frame_schema: LirFrameSchemaFacts,
    pub boundary_map: LirBoundaryMapFacts,
    pub resume_state_map: LirResumeStateMapFacts,
    pub source_statement_count: usize,
    pub continuation_object: LirContinuationObjectKey,
    pub resume_packings: Vec<LirResumePackingKey>,
}

/// Step type shell and case contracts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirStepTypeFacts {
    pub step_schema: LirStepSchemaKey,
    pub invoke_args_tuple_ty: TypeId,
    pub complete_ty: TypeId,
    pub continuation_obj_ty: TypeId,
    pub cases: Vec<LirStepCaseFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirStepCaseFacts {
    pub case_tag: LirCaseKey,
    pub payload_tuple_ty: TypeId,
    pub continuation_schema: LirContinuationSchemaKey,
}

/// Dynamic-invoke source location.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirDynamicInvokeSource {
    Boundary {
        boundary_id: LirBoundaryKey,
    },
    ControlSourceSlice {
        source_slice: LirSourceSliceKey,
        statement_index: u32,
    },
    PlainCallSite {
        source_slice: LirSourceSliceKey,
        statement_index: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirDynamicInvokeCarrierKind {
    ClosureObject,
    FunPtr,
    VirtualReceiver,
    InterfaceReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirDynamicInvokeCarrierContract {
    pub kind: LirDynamicInvokeCarrierKind,
    pub source_ty: Option<TypeId>,
    pub dispatch: Option<LirDispatchKey>,
}

/// Backend-neutral dynamic-invoke contract for a call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirDynamicInvokeContract {
    pub owner_callable: StableLirCallableKey,
    pub owner_step_schema: Option<LirStepSchemaKey>,
    pub site_id: SiteId,
    pub source: LirDynamicInvokeSource,
    pub call: LirCallSiteContract,
    pub carrier: LirDynamicInvokeCarrierContract,
    pub arg_count: usize,
    pub target_body_versions: Vec<BodyVersionKey>,
}

/// Dispatch owner/slot selection published before backend layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirDispatchContract {
    pub owner_callable: StableLirCallableKey,
    pub site_id: SiteId,
    pub kind: LirCallSiteKind,
    pub owner_fqn: String,
    pub member_name: String,
    pub member_fqn: String,
    pub receiver_ty: TypeId,
    pub explicit_arg_count: usize,
    pub method_slot: u32,
    pub interface_id: Option<u64>,
    pub candidate_targets: Vec<StableLirCallableKey>,
}

/// Effect-family resume packing helper.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirResumePackingFacts {
    pub interface_id: LirResumePackingKey,
    pub effect_fqn: String,
    pub effect_type_args: Vec<TypeId>,
    pub return_step_schema: LirStepSchemaKey,
    pub methods: Vec<LirResumeMethodFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirResumeMethodFacts {
    pub case_tag: LirCaseKey,
    pub continuation_schema: LirContinuationSchemaKey,
    pub resume_tuple_ty: TypeId,
    pub answer_ty: TypeId,
    pub out_step_schema: LirStepSchemaKey,
    pub surface_ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirContinuationResumeBody {
    ResumeCapturedState,
    OneShotRuntimeErrorPublication,
    Unreachable,
}

/// Continuation object and per-case resume publication.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirContinuationObjectFacts {
    pub object_id: LirContinuationObjectKey,
    pub owner_body_version: BodyVersionKey,
    pub continuation_obj_ty: TypeId,
    pub implemented_packings: Vec<LirResumePackingKey>,
    pub surface_resumes: Vec<LirContinuationResumeFacts>,
    pub methods: Vec<LirContinuationMethodFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirContinuationResumeFacts {
    pub case_tag: LirCaseKey,
    pub continuation_schema: LirContinuationSchemaKey,
    pub resume_tuple_ty: TypeId,
    pub answer_ty: TypeId,
    pub out_step_schema: LirStepSchemaKey,
    pub surface_ty: TypeId,
    pub body: LirContinuationResumeBody,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirContinuationMethodFacts {
    pub packing_interface_id: LirResumePackingKey,
    pub resume: LirContinuationResumeFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirSurfaceResumeDispatchSourceKind {
    ContinuationObjectMethod,
    ResumeBoundaryOnly,
    HandleContinuationBinderOnly,
    OwnerTrampolineMixed,
    Unreachable,
}

/// Surface-resume dispatch inventory and wrapper projection completeness.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirSurfaceResumeDispatchFacts {
    pub continuation_schema: LirContinuationSchemaKey,
    pub resume_tuple_ty: TypeId,
    pub answer_ty: TypeId,
    pub out_step_schema: LirStepSchemaKey,
    pub source_kind: LirSurfaceResumeDispatchSourceKind,
    pub publication_count: usize,
    pub wrapper_projection_count: usize,
}
