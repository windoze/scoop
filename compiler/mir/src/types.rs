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
    Unit,
    Int,
    /// Unsigned 64-bit integer (`UInt`, spec 11.2; same machine word
    /// as `Int`, mapped to `i64` at LIR).
    UInt,
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

/// Typed identity reserved for variance bridges. M11's variance gate fills
/// this arena; keeping it distinct now prevents adapters from being confused
/// with source closure classes.
#[derive(Debug)]
pub struct ClosureAdapter {
    pub class: ClosureClassId,
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

/// Adapter used after a runtime `Any`/interface-to-function check. Its source
/// signature is discovered from the captured closure's TypeDescriptor bridge
/// table, while its exposed invoke ABI is exactly `target`.
#[derive(Debug)]
pub struct DynamicClosureAdapter {
    pub class: ClosureClassId,
    pub target: FunctionTypeId,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    /// Fixed after all type parameters have been resolved and this MIR
    /// type entity has a complete concrete field list.
    pub gc_free: bool,
    pub representation: StructRepresentation,
}

#[derive(Debug)]
pub enum StructRepresentation {
    Declared {
        c_layout: Option<CLayout>,
        interior_mutable: bool,
        fields: Vec<Field>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl StructDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is the mangled instance name (e.g. `Option$I`).
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    /// True exactly when every fully specialized variant is GC-free.
    pub gc_free: bool,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub name: String,
    /// GC-free classification of this fully specialized variant.
    pub gc_free: bool,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<Field>,
}

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
    pub modifier: ClassModifier,
    pub name: String,
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
    Int,
    UInt,
    Boolean,
    String,
    Array { element: Type },
    MutableArray { element: Type },
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
    /// Signature-only method declarations in itable-slot order.
    pub methods: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_state_ids_are_nonzero_by_construction() {
        assert_eq!(CoroutineSuspendStateId::new(0), None);
        assert_eq!(
            CoroutineSuspendStateId::new(1).map(CoroutineSuspendStateId::get),
            Some(1)
        );
        assert_eq!(
            CoroutineSuspendStateId::new(u32::MAX).map(CoroutineSuspendStateId::get),
            Some(u32::MAX)
        );
    }

    #[test]
    fn coroutine_frame_state_encodings_do_not_overlap() {
        let first = CoroutineSuspendStateId::new(1).expect("one is nonzero");
        let last = CoroutineSuspendStateId::new(u32::MAX).expect("u32::MAX is nonzero");
        let initial =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Initial).raw_bits();
        let running =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Running).raw_bits();
        let completed =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Completed).raw_bits();
        let first_suspended =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(first))
                .raw_bits();
        let last_suspended =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(last))
                .raw_bits();
        let first_failure =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(first))
                .raw_bits();
        let last_failure =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(last))
                .raw_bits();

        assert_eq!(initial, 0);
        assert_eq!(first_suspended, 1);
        assert_eq!(last_suspended, u64::from(u32::MAX));
        assert!(last_suspended < last_failure);
        assert!(last_failure <= first_failure);
        assert!(first_failure < completed);
        assert!(completed < running);
    }

    #[test]
    fn machine_scalar_values_have_one_total_semantic_kind() {
        let site = CoroutineSuspendStateId::new(1).expect("one is nonzero");
        let cases = [
            (MachineScalarValue::EnumTag(7), MachineScalarKind::EnumTag),
            (
                MachineScalarValue::InitializationOutcome(InitializationOutcome::Ready),
                MachineScalarKind::InitializationOutcome,
            ),
            (
                MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(site)),
                MachineScalarKind::CoroutineFrameState,
            ),
            (
                MachineScalarValue::CoroutineAdapterState(CoroutineAdapterState::Waiting),
                MachineScalarKind::CoroutineAdapterState,
            ),
            (
                MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
                MachineScalarKind::ForeignCallbackStatus,
            ),
            (
                MachineScalarValue::PointerElementOffset(9),
                MachineScalarKind::PointerElementOffset,
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.kind(), expected);
            assert_eq!(
                Expr::machine_scalar(value).ty,
                Type::MachineScalar(expected)
            );
        }
        assert_eq!(
            Expr::enum_tag(Expr::unit()).ty,
            Type::MachineScalar(MachineScalarKind::EnumTag)
        );
    }

    #[test]
    #[should_panic(expected = "machine scalar equality operands have the same semantic kind")]
    fn machine_scalar_equality_rejects_mismatched_kinds() {
        let _ = Expr::machine_eq(
            Expr::machine_scalar(MachineScalarValue::EnumTag(0)),
            MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
        );
    }
}
