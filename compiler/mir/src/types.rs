use super::*;
use std::num::NonZeroU32;

/// Compiler-owned scalar domains that never denote a Scoop source integer.
///
/// Keeping the semantic role in the type prevents enum discriminants,
/// runtime protocol outcomes, and coroutine synchronization words from
/// acquiring source `Int`/`UInt` operations, boxing, or FFI behavior merely
/// because their current physical representation is an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarKind {
    EnumTag,
    InitializationOutcome,
    CoroutineFrameState,
    CoroutineAdapterState,
    ForeignCallbackStatus,
    PointerElementOffset,
}

impl MachineScalarKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::EnumTag => "enum-tag",
            Self::InitializationOutcome => "initialization-outcome",
            Self::CoroutineFrameState => "coroutine-frame-state",
            Self::CoroutineAdapterState => "coroutine-adapter-state",
            Self::ForeignCallbackStatus => "foreign-callback-status",
            Self::PointerElementOffset => "pointer-element-offset",
        }
    }

    pub const fn is_atomic_state(self) -> bool {
        matches!(
            self,
            Self::CoroutineFrameState | Self::CoroutineAdapterState
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InitializationOutcome {
    RunInitializer,
    Ready,
    Failed,
    Cycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoroutineSuspendStateId(NonZeroU32);

impl CoroutineSuspendStateId {
    pub fn new(raw: u32) -> Option<Self> {
        NonZeroU32::new(raw).map(Self)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl std::fmt::Display for CoroutineSuspendStateId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoroutineFrameState {
    Initial,
    Running,
    Completed,
    /// A successful resume entry, keyed by the one-based suspension site.
    Suspended(CoroutineSuspendStateId),
    /// An exceptional resume entry for the same one-based suspension site.
    ResumeFailure(CoroutineSuspendStateId),
}

impl std::fmt::Display for CoroutineFrameState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initial => formatter.write_str("initial"),
            Self::Running => formatter.write_str("running"),
            Self::Completed => formatter.write_str("completed"),
            Self::Suspended(site) => write!(formatter, "suspended.{site}"),
            Self::ResumeFailure(site) => write!(formatter, "resume_failure.{site}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoroutineAdapterState {
    Registering,
    Waiting,
    CompletingSuccess,
    CompletingFailure,
    LatchedSuccess,
    LatchedFailure,
    Consumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForeignCallbackStatus {
    Returned,
    Threw,
}

/// A semantic constant in one compiler-owned scalar domain.
///
/// The variant determines the kind, so a value cannot carry a contradictory
/// parallel kind field. `raw_bits` is the frozen runtime encoding consumed by
/// lower stages; it does not turn the value into a source unsigned integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarValue {
    EnumTag(u32),
    InitializationOutcome(InitializationOutcome),
    CoroutineFrameState(CoroutineFrameState),
    CoroutineAdapterState(CoroutineAdapterState),
    ForeignCallbackStatus(ForeignCallbackStatus),
    PointerElementOffset(u64),
}

impl MachineScalarValue {
    pub const fn kind(self) -> MachineScalarKind {
        match self {
            Self::EnumTag(_) => MachineScalarKind::EnumTag,
            Self::InitializationOutcome(_) => MachineScalarKind::InitializationOutcome,
            Self::CoroutineFrameState(_) => MachineScalarKind::CoroutineFrameState,
            Self::CoroutineAdapterState(_) => MachineScalarKind::CoroutineAdapterState,
            Self::ForeignCallbackStatus(_) => MachineScalarKind::ForeignCallbackStatus,
            Self::PointerElementOffset(_) => MachineScalarKind::PointerElementOffset,
        }
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::EnumTag(tag) => tag as u64,
            Self::InitializationOutcome(outcome) => match outcome {
                InitializationOutcome::RunInitializer => 0,
                InitializationOutcome::Ready => 1,
                InitializationOutcome::Failed => 2,
                InitializationOutcome::Cycle => 3,
            },
            Self::CoroutineFrameState(state) => match state {
                CoroutineFrameState::Initial => 0,
                CoroutineFrameState::Running => u64::MAX,
                CoroutineFrameState::Completed => u64::MAX - 1,
                CoroutineFrameState::Suspended(site) => site.get() as u64,
                CoroutineFrameState::ResumeFailure(site) => u64::MAX - site.get() as u64 - 1,
            },
            Self::CoroutineAdapterState(state) => match state {
                CoroutineAdapterState::Registering => 0,
                CoroutineAdapterState::Waiting => 1,
                CoroutineAdapterState::CompletingSuccess => 2,
                CoroutineAdapterState::CompletingFailure => 3,
                CoroutineAdapterState::LatchedSuccess => 4,
                CoroutineAdapterState::LatchedFailure => 5,
                CoroutineAdapterState::Consumed => 6,
            },
            Self::ForeignCallbackStatus(status) => match status {
                ForeignCallbackStatus::Returned => 0,
                ForeignCallbackStatus::Threw => 1,
            },
            Self::PointerElementOffset(offset) => offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Compiler-owned GC values; they cannot be named by Scoop source.
    Context(ContextStorageType),
    Unit,
    /// One exact Scoop source integer. Its nominal owner is already canonical
    /// (transparent aliases have disappeared before MIR).
    Integer(IntegerKind),
    /// An internal typed scalar, disjoint from every source integer type.
    MachineScalar(MachineScalarKind),
    Boolean,
    String,
    Struct(StructId),
    /// A reference type declared with `class`.
    Class(ClassId),
    /// An interface type (dispatch through itables, impl spec 2.9).
    Interface(InterfaceId),
    /// The root of all types; boxed value types live behind it.
    Any,
    Tuple(Vec<Type>),
    /// Concrete managed function signature. Function values have reference
    /// representation; closure classes are materialized by M11 conversion.
    Function(FunctionTypeId),
    /// Typed raw data pointer; representation is one native pointer word.
    Ptr(Box<Type>),
    /// Typed C function pointer; identity includes its exact signature.
    FunPtr(FunctionTypeId),
    /// An instantiated enum type (including `Option<T>` since M4).
    Enum(EnumId, Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug)]
pub struct ClosureClass {
    pub name: String,
    pub function_type: FunctionTypeId,
    pub invoke: ClosureInvokeFunctionId,
    pub captures: Vec<Field>,
    /// Function-type views supported by this exact closure class. Each slot
    /// is a typed forwarding entry whose ABI is `target`.
    pub bridges: Vec<FunctionBridge>,
}

#[derive(Debug)]
pub struct FunctionBridge {
    pub target: FunctionTypeId,
    pub function: FunctionId,
}

#[derive(Debug)]
pub struct ClosureInvokeFunction {
    pub function: FunctionId,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    /// Fixed after all type parameters have been resolved and this MIR
    /// type entity has a complete concrete field list.
    pub gc_free: bool,
    pub representation: StructRepresentation,
}

#[derive(Debug)]
pub enum StructRepresentation {
    Declared {
        c_layout: Option<MirCLayoutContract>,
        c_abi: StructCAbi,
        interior_mutable: bool,
        fields: Vec<DeclaredStructField>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructCAbi {
    SourceRepresentation,
    UInt64Field {
        field: scoop_identity::PersistentFieldId,
    },
}

impl StructDef {
    /// MIR type whose physical layout is defined by this declaration.
    pub fn physical_type(&self, id: StructId) -> Type {
        match &self.representation {
            StructRepresentation::Declared { .. } => Type::Struct(id),
            StructRepresentation::Intrinsic(representation) => match representation {
                IntrinsicTypeRepresentation::Integer(kind) => Type::Integer(*kind),
                IntrinsicTypeRepresentation::Boolean => Type::Boolean,
                IntrinsicTypeRepresentation::Char => Type::Struct(id),
                IntrinsicTypeRepresentation::Ptr { pointee } => {
                    Type::Ptr(Box::new(pointee.clone()))
                }
                IntrinsicTypeRepresentation::FunPtr { signature } => Type::FunPtr(*signature),
                IntrinsicTypeRepresentation::String
                | IntrinsicTypeRepresentation::Array { .. }
                | IntrinsicTypeRepresentation::MutableArray { .. } => {
                    unreachable!("the MIR intrinsic registry fixes physical declaration kinds")
                }
            },
        }
    }

    pub fn declared_fields(&self) -> &[DeclaredStructField] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<DeclaredStructField> {
        match &mut self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl MirCLayoutValue {
    pub const fn bytes(self) -> Option<u8> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::A1 => "1",
            Self::A2 => "2",
            Self::A4 => "4",
            Self::A8 => "8",
            Self::A16 => "16",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirCLayoutContract {
    pub aligned: MirCLayoutValue,
    pub packed: MirCLayoutValue,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// Source-declared struct field retained with its persistent identity for
/// target layout and native ABI projection.
#[derive(Debug)]
pub struct DeclaredStructField {
    pub identity: PersistentFieldId,
    pub name: String,
    pub ty: Type,
}

mod enumeration;
pub use enumeration::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// A class definition with its dispatch layout fixed by mir-lower
/// (impl spec 2.9).
#[derive(Debug)]
pub struct ClassDef {
    pub release_policy: ReleasePolicy,
    pub modifier: ClassModifier,
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    pub representation: ClassRepresentation,
    pub interfaces: Vec<InterfaceId>,
    /// Ordinary virtual methods in vtable order (overrides share the base
    /// slot). Empty vtables are valid and have no implicit prefix.
    pub vtable: Vec<TableSlot>,
    /// itable entries, one per implemented interface (pointer-keyed
    /// lookup at runtime).
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub enum ClassRepresentation {
    Declared {
        /// Constructor properties in flattened base-first order.
        fields: Vec<Field>,
        base_class: Option<ClassId>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl ClassDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared { base_class, .. } => *base_class,
            ClassRepresentation::Intrinsic(_) => None,
        }
    }
}

/// The exact compiler representation selected upstream for this fully
/// specialized nominal type. Family variants carry their MIR element type.
#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicTypeRepresentation {
    Integer(IntegerKind),
    Boolean,
    Char,
    String,
    Array {
        element: Type,
    },
    MutableArray {
        element: Type,
    },
    /// Intrinsic `Ptr<T>` declaration provenance. Canonical values still use
    /// [`Type::Ptr`], never this declaration as an ordinary struct payload.
    Ptr {
        pointee: Type,
    },
    /// Intrinsic `FunPtr<F>` declaration provenance. Canonical values still
    /// use [`Type::FunPtr`].
    FunPtr {
        signature: FunctionTypeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Immutable,
    Mutable,
}

pub fn array_type<'a>(module: &'a Module, ty: &Type) -> Option<(ArrayKind, &'a Type)> {
    let Type::Class(class) = ty else {
        return None;
    };
    match &module.classes[*class].representation {
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
            Some((ArrayKind::Immutable, element))
        }
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray { element }) => {
            Some((ArrayKind::Mutable, element))
        }
        ClassRepresentation::Declared { .. }
        | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => None,
        ClassRepresentation::Intrinsic(_) => {
            unreachable!("the intrinsic registry fixes declaration targets")
        }
    }
}

#[derive(Debug)]
pub enum TableSlot {
    Function(FunctionId),
    External(ExternalCallableUseId),
    Runtime(RuntimeFn),
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: InterfaceId,
    pub slots: Vec<TableSlot>,
}

#[derive(Debug)]
pub struct InterfaceDef {
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    /// Direct exact parent interfaces from concrete HIR.
    pub parents: Vec<InterfaceId>,
    /// Complete method signatures in itable-slot order.
    pub methods: Vec<InterfaceMethod>,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethod {
    pub name: String,
    pub gc_effect: GcEffect,
    pub parameters: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[cfg(test)]
mod tests;
