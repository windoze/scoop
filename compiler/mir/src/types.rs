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
    pub receiver: Option<Type>,
    pub is_suspend: bool,
    pub parameter_types: Vec<Type>,
    pub return_type: Type,
}

impl FunctionType {
    /// Source arguments in invocation order; the closure environment is separate.
    pub fn parameters(&self) -> impl Iterator<Item = &Type> {
        self.receiver.iter().chain(&self.parameter_types)
    }

    pub fn arity(&self) -> usize {
        usize::from(self.receiver.is_some()) + self.parameter_types.len()
    }
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

mod nominals;
pub use nominals::*;
mod enumeration;
pub use enumeration::*;

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
    /// Original declaration identity; names and erased signatures are not slots.
    pub identity: scoop_identity::PersistentDispatchSlotId,
    pub name: String,
    pub gc_effect: GcEffect,
    pub parameters: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct Local {
    pub debug_origin: DebugOrigin,
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[cfg(test)]
mod tests;
