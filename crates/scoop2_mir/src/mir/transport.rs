//! transport / metadata 契约层。
//!
//! 每个 Rvalue/Statement/Terminator 携带的语义契约结构体，用于后端（layout/codegen）
//! 在不回查 HIR 的前提下获得值穿越边界的装箱/擦除/trace/copy/drop 信息。
//!
//! 设计原则（与参考实现 scoopc_mir 一致）：
//! - 只表达语言级语义契约，不含 vtable slot/itable id/LLVM statepoint（后端的事）；
//! - NicheStorage 逻辑简化为 Option<T> 的 trace 判定（标量 niche → false，其余 → true）。

use scoop2_base::Span;
use scoop2_hir::ty::{EffectRow, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::LocalId;

// ---------------------------------------------------------------------------
// transport kind / boxing
// ---------------------------------------------------------------------------

/// 值穿越边界时的 backend-agnostic 分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirTransportKind {
    Scalar,
    Reference,
    Tuple,
    Struct,
    EnumPayload,
    ClosureEnv,
    ArrayElement,
    EffectPayload,
    FunctionValue,
    Unknown,
}

/// 聚合值须装箱的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBoxingReason {
    AnyErasure,
    RefErasure,
    EffectPayload,
    ClosureCapture,
    ArrayElement,
    FunctionValueAdapter,
}

/// 装箱意图（source_ty → target_ty + reason）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirBoxingIntent {
    pub source_ty: TypeId,
    pub target_ty: Option<TypeId>,
    pub reason: MirBoxingReason,
}

/// copy/drop/trace 义务。
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

// ---------------------------------------------------------------------------
// ValueTransportMetadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// AggregateTransportMetadata
// ---------------------------------------------------------------------------

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

impl AggregateTransportMetadata {
    pub fn is_tuple_or_struct(&self) -> bool {
        matches!(
            self.kind,
            AggregateTransportKind::Tuple | AggregateTransportKind::Struct
        )
    }

    pub fn fields_have_no_boxing(&self) -> bool {
        self.fields
            .iter()
            .all(|field| field.transport.boxing.is_none())
    }
}

// ---------------------------------------------------------------------------
// ClosureEnvTransportMetadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ArrayElementTransportMetadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// GcIntrinsicTransportMetadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// CallAbiHandoffMetadata + CallTransportMetadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// site-level metadata structs (Dispatch / Perform / Handle / Resume / ClassCtor / Member / RuntimeType)
// ---------------------------------------------------------------------------

/// virtual/interface dispatch 的语言级 metadata（不含 vtable slot）。
#[derive(Debug, Clone)]
pub struct DispatchMetadata {
    pub owner_fqn: String,
    pub member_name: String,
    pub member_fqn: String,
    pub member_decl_span: Option<Span>,
    pub receiver_ty: TypeId,
    pub stable_candidate_keys: Vec<StableInstanceKey>,
    pub stable_template_key: Option<StableTemplateKey>,
    pub generic_type_args: Vec<TypeId>,
    pub generic_eff_args: Vec<EffectRow>,
}

/// perform 调用点的 typed contract。
#[derive(Debug, Clone)]
pub struct PerformMetadata {
    pub effect_ty: TypeId,
    pub op_type_args: Vec<TypeId>,
    pub result_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub payload_transport: Vec<ValueTransportMetadata>,
    pub arg_mapping: Vec<usize>,
}

/// handle 站点的 typed contract。
#[derive(Debug, Clone)]
pub struct HandleMetadata {
    pub result_ty: TypeId,
    pub body_result_ty: TypeId,
    pub finally_result_ty: Option<TypeId>,
    /// handle 结果 local（body/arm 都写入它；escape continuation 的边界克隆
    /// 用它构造 `Return(Step::Complete(result_local))`）。
    pub result_local: crate::mir::LocalId,
}

/// Continuation.resume 的语义 metadata。
#[derive(Debug, Clone)]
pub struct ResumeMetadata {
    pub continuation_ty: TypeId,
    pub resume_ty: TypeId,
    pub answer_ty: TypeId,
    pub return_ty: TypeId,
    pub out_effects: EffectRow,
    pub runtime_error_effect_ty: Option<TypeId>,
    pub suspends_outward: bool,
}

/// class ctor call 的 selected ctor / ordered-args contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassCtorCallMetadata {
    pub target_init_class_fqn: String,
    pub selected_ctor_span: Option<Span>,
    pub ordered_param_count: usize,
    /// 构造器的 stable template key（含 class FQN + overload sig）。
    /// 供分离编译 / 跨模块构造器引用稳定性使用。None = 尚未计算。
    pub stable_template_key: Option<StableTemplateKey>,
}

/// 成员访问的语言级 metadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessMetadata {
    pub name: String,
    pub receiver_ty: TypeId,
    pub resolved: Option<MemberTarget>,
    pub hidden_effects: EffectRow,
}

/// 已解析成员的稳定目标种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberTarget {
    Value { fqn: String },
    Fun { fqn: String },
    ExtensionValue { fqn: String },
    ExtensionFun { fqn: String },
}

/// handle arm 的显式语义 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerArmKind {
    NonResuming,
    EscapeContinuation,
}

/// handle arm 的 typed contract。
#[derive(Debug, Clone)]
pub struct HandlerArm {
    pub op_fqn: String,
    pub op_type_args: Vec<TypeId>,
    pub binder_count: usize,
    pub binder_locals: Vec<LocalId>,
    pub continuation_local: Option<LocalId>,
    pub handled_effect_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub body_ty: TypeId,
    pub kind: HandlerArmKind,
}

/// 运行期类型检查 descriptor key。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTypeDescriptorKey {
    pub ty: TypeId,
    pub kind: RuntimeTypeDescriptorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTypeDescriptorKind {
    Any,
    String,
    Nominal { fqn: String, kind: Option<String> },
    Function,
    Option,
    Tuple,
    Value,
    TypeParam,
    StarProjection,
    Union,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTypeStaticFold {
    AlwaysTrue,
    AlwaysFalse,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTypeParameterizedMatch {
    None,
    Nominal {
        type_args: Vec<TypeId>,
        effect_arg: Option<EffectRow>,
    },
    Function {
        receiver: Option<TypeId>,
        params: Vec<TypeId>,
        return_ty: TypeId,
        effects: EffectRow,
        effects_closed: bool,
    },
    Option {
        payload_ty: TypeId,
    },
    Tuple {
        element_tys: Vec<TypeId>,
    },
    Union {
        variants: Vec<TypeId>,
    },
    StarProjection {
        read_ty: TypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTypeTestMetadata {
    pub source_ty: TypeId,
    pub target_ty: TypeId,
    pub descriptor: RuntimeTypeDescriptorKey,
    pub static_fold: RuntimeTypeStaticFold,
    pub parameterized: RuntimeTypeParameterizedMatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCastFailure {
    Panic { message: String },
    ReturnNone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCastResult {
    Target { ty: TypeId },
    Option { option_ty: TypeId, some_ty: TypeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCastMetadata {
    pub test: RuntimeTypeTestMetadata,
    pub failure: RuntimeCastFailure,
    pub result: RuntimeCastResult,
}

// ---------------------------------------------------------------------------
// continuation route publication (member write)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PatternBindingStep {
    TupleIndex(usize),
    VariantField { variant: String, field_index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredContinuationValueRoute {
    pub source_local: LocalId,
    pub source_ty: TypeId,
    pub path: Vec<PatternBindingStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredContinuationRoutePublication {
    None,
    Unique(StoredContinuationValueRoute),
    Ambiguous,
}

// ---------------------------------------------------------------------------
// stable keys（基于规范文本 + scope hash）
// ---------------------------------------------------------------------------

/// 稳定模板 key：cone + namespace + owner_path + decl_kind + overload_sig。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableTemplateKey {
    pub canonical: String,
    pub hash: String,
}

/// 稳定实例 key：template key + canonical type/effect args。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableInstanceKey {
    pub template: StableTemplateKey,
    pub canonical_type_args: Vec<String>,
    pub canonical_effect_args: Vec<String>,
    pub hash: String,
}

/// 顶层值/函数引用的 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelRef {
    pub fqn: String,
    pub hidden_effects: EffectRow,
    pub stable_template_key: Option<StableTemplateKey>,
    pub stable_instance_key: Option<StableInstanceKey>,
    pub generic_type_args: Vec<TypeId>,
    pub generic_eff_args: Vec<EffectRow>,
}

// ---------------------------------------------------------------------------
// 分类 helper（transport kind / trace / requirements / boxing）
// ---------------------------------------------------------------------------

/// option niche 的简化存储分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NicheStorage {
    Pointer,
    U8,
    None,
}

/// Option<T> 的 niche 存储判定（简化）：
/// - inner 为标量 Bool → U8 niche（不 trace）；
/// - inner 为引用 / nominal → Pointer（trace）；
/// - 其余 → None（保守 trace）。
fn option_niche_storage(types: &TypeStore, inner: TypeId) -> NicheStorage {
    match types.kind(inner) {
        TypeKind::Value(ValueTypeKind::Bool) => NicheStorage::U8,
        TypeKind::Value(ValueTypeKind::Unit) => NicheStorage::U8,
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection => NicheStorage::Pointer,
        TypeKind::Value(ValueTypeKind::Nominal(_)) => NicheStorage::Pointer,
        TypeKind::Value(ValueTypeKind::Tuple(_)) => NicheStorage::None,
        _ => NicheStorage::U8,
    }
}

/// 判断某类型是否需要 GC trace（与 layout/codegen 一致）。
pub fn mir_transport_trace_requirement_for_type(types: &TypeStore, ty: TypeId) -> bool {
    // Option<T>：按 inner 的 niche 存储判定（Option 现为 value nominal，走 FQN 判定）。
    if let Some(inner) = types
        .nominal_args_of_fqn(ty, types.option_fqn())
        .and_then(|args| args.first().copied())
    {
        return match option_niche_storage(types, inner) {
            NicheStorage::Pointer => true,
            NicheStorage::U8 => false,
            NicheStorage::None => true,
        };
    }
    match types.kind(ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection => true,
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements
            .iter()
            .any(|element| mir_transport_trace_requirement_for_type(types, *element)),
        TypeKind::Value(ValueTypeKind::Nominal(_)) => true,
        TypeKind::Value(
            ValueTypeKind::Unit
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_),
        ) => false,
        TypeKind::Nothing => false,
    }
}

/// 判断某类型是否为聚合 transport 类型。
pub fn mir_is_aggregate_transport_ty(types: &TypeStore, ty: TypeId) -> bool {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Tuple(_)) => true,
        TypeKind::Value(ValueTypeKind::Nominal(_)) => true,
        _ => false,
    }
}

/// 从 TypeKind 判定 transport kind。
///
/// `enum_fqns`：函数体内遇到的 enum FQN 集合（用于区分 struct vs enum nominal）。
/// None 表示不可用（保守判为 Struct）。
pub fn mir_transport_kind_for_ty(
    types: &TypeStore,
    ty: TypeId,
    enum_fqns: &std::collections::HashSet<scoop2_base::Symbol>,
) -> MirTransportKind {
    use scoop2_hir::ty::RefTypeKind;
    // Option<T> → EnumPayload（Option 现为 value nominal，走 FQN 判定）。
    if types.is_nominal_with_fqn(ty, types.option_fqn()) {
        return MirTransportKind::EnumPayload;
    }
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => MirTransportKind::FunctionValue,
        TypeKind::Ref(_) => MirTransportKind::Reference,
        TypeKind::Value(ValueTypeKind::Tuple(_)) => MirTransportKind::Tuple,
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            if enum_fqns.contains(&n.fqn) {
                MirTransportKind::EnumPayload
            } else {
                MirTransportKind::Struct
            }
        }
        TypeKind::Value(_) => MirTransportKind::Scalar,
        TypeKind::Nothing => MirTransportKind::Scalar,
        TypeKind::Param(_) | TypeKind::StarProjection => MirTransportKind::Unknown,
    }
}

/// 从类型计算 transport requirements。
pub fn mir_transport_requirements(types: &TypeStore, ty: TypeId) -> MirTransportRequirements {
    let trace = mir_transport_trace_requirement_for_type(types, ty);
    MirTransportRequirements {
        trace,
        copy: true,
        drop: trace || mir_is_aggregate_transport_ty(types, ty),
    }
}

/// 从类型计算 ValueTransportMetadata（无 boxing）。
pub fn value_transport(
    types: &TypeStore,
    enum_fqns: &std::collections::HashSet<scoop2_base::Symbol>,
    source_ty: TypeId,
) -> ValueTransportMetadata {
    ValueTransportMetadata {
        source_ty,
        kind: mir_transport_kind_for_ty(types, source_ty, enum_fqns),
        requirements: mir_transport_requirements(types, source_ty),
        boxing: None,
    }
}

/// erasure boxing reason（source → target 时的装箱原因）。
pub fn erasure_boxing_reason(
    types: &TypeStore,
    any_ty: TypeId,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<MirBoxingReason> {
    if source_ty == target_ty || !matches!(types.kind(source_ty), TypeKind::Value(_)) {
        return None;
    }
    if target_ty == any_ty {
        return Some(MirBoxingReason::AnyErasure);
    }
    match types.kind(target_ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection => {
            Some(MirBoxingReason::RefErasure)
        }
        TypeKind::Value(_) => None,
        TypeKind::Nothing => None,
    }
}

/// 值擦除到 target_ty 时的 transport metadata（带 boxing）。
pub fn value_erasure_transport(
    types: &TypeStore,
    enum_fqns: &std::collections::HashSet<scoop2_base::Symbol>,
    any_ty: TypeId,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<ValueTransportMetadata> {
    let reason = erasure_boxing_reason(types, any_ty, source_ty, target_ty)?;
    Some(ValueTransportMetadata {
        source_ty,
        kind: mir_transport_kind_for_ty(types, source_ty, enum_fqns),
        requirements: mir_transport_requirements(types, source_ty),
        boxing: Some(MirBoxingIntent {
            source_ty,
            target_ty: Some(target_ty),
            reason,
        }),
    })
}
