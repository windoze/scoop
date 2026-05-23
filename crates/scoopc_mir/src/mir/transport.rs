use crate::span::Span;
use crate::ty::TypeId;
use crate::ty::layout::{NicheDomain, NicheStorage};
use crate::ty::{TypeKind, TypeStore, ValueTypeKind, is_builtin_scalar_nominal_value_type};

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

/// Shared transport trace requirement rule for MIR/authored transport metadata.
///
/// `Option<T>` must follow the same physical layout choice used by type/layout/codegen:
/// tagged-union fallback always carries a GC pointer slot, while niche-optimized `Option<Bool>`
/// stays scalar and must not claim traceability.
pub fn mir_transport_trace_requirement_for_type(types: &TypeStore, ty: TypeId) -> bool {
    match types.kind(ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection(_) => true,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            match option_transport_niche_storage(types, *inner) {
                Some(NicheStorage::Pointer) => true,
                Some(NicheStorage::U8) => false,
                None => true,
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements
            .iter()
            .any(|element| mir_transport_trace_requirement_for_type(types, *element)),
        TypeKind::Value(ValueTypeKind::Nominal(_))
            if is_builtin_scalar_nominal_value_type(types, ty) =>
        {
            false
        }
        // Nominal value fields are not in `TypeKind`; keep the contract conservative and let
        // later layout/codegen query declaration metadata instead of guessing non-builtin shape.
        TypeKind::Value(ValueTypeKind::Nominal(_)) => true,
        TypeKind::Value(
            ValueTypeKind::Unit
            | ValueTypeKind::Nothing
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_),
        ) => false,
    }
}

fn option_transport_niche_storage(types: &TypeStore, inner: TypeId) -> Option<NicheStorage> {
    let mut domain = transport_niche_domain(types, inner)?;
    let _none_value = domain.take_one()?;
    if domain.storage == NicheStorage::Pointer {
        domain.next = domain.end;
    }
    Some(domain.storage)
}

fn transport_niche_domain(types: &TypeStore, ty: TypeId) -> Option<NicheDomain> {
    match types.kind(ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection(_) => Some(NicheDomain {
            storage: NicheStorage::Pointer,
            next: 0,
            end: 1,
        }),
        TypeKind::Value(ValueTypeKind::Bool) => Some(NicheDomain {
            storage: NicheStorage::U8,
            next: 2,
            end: 256,
        }),
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let mut domain = transport_niche_domain(types, *inner)?;
            let _none_value = domain.take_one()?;
            if domain.storage == NicheStorage::Pointer {
                domain.next = domain.end;
            }
            (!domain.is_empty()).then_some(domain)
        }
        TypeKind::Value(
            ValueTypeKind::Unit
            | ValueTypeKind::Nothing
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_)
            | ValueTypeKind::Tuple(_)
            | ValueTypeKind::Nominal(_),
        ) => None,
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

#[cfg(test)]
mod tests {
    use super::mir_transport_trace_requirement_for_type;
    use crate::ty::TypeStore;

    #[test]
    fn option_transport_trace_requirement_tracks_layout_representation() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();

        assert!(mir_transport_trace_requirement_for_type(
            &types,
            builtins.string
        ));
        assert!(!mir_transport_trace_requirement_for_type(
            &types,
            builtins.bool_
        ));

        let option_int = types.ty_option(builtins.int);
        let option_bool = types.ty_option(builtins.bool_);
        let option_string = types.ty_option(builtins.string);
        let nested_option_bool = types.ty_option(option_bool);
        let nested_option_string = types.ty_option(option_string);

        assert!(
            mir_transport_trace_requirement_for_type(&types, option_int),
            "tagged-union Option<Int> must stay traceable because its runtime layout carries a GC slot"
        );
        assert!(
            mir_transport_trace_requirement_for_type(&types, option_string),
            "pointer-niche Option<String> must stay traceable"
        );
        assert!(
            mir_transport_trace_requirement_for_type(&types, nested_option_string),
            "nested Option<Option<String>> exhausts pointer niche and falls back to tagged union"
        );
        assert!(
            !mir_transport_trace_requirement_for_type(&types, option_bool),
            "Option<Bool> keeps scalar niche layout and must not publish trace requirement"
        );
        assert!(
            !mir_transport_trace_requirement_for_type(&types, nested_option_bool),
            "nested Option<Option<Bool>> still uses U8 niche and must stay non-traceable"
        );
    }
}
