use super::*;

pub type GlobalId = Idx<Global>;
pub type InitializationUnitId = Idx<InitializationUnit>;
pub type LocalId = Idx<Local>;
pub type TempId = Idx<Temp>;
pub type BlockId = Idx<BasicBlock>;
pub type EnumDefId = Idx<EnumDef>;
pub type StructDefId = Idx<StructDef>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type NativeGlobalId = Idx<NativeGlobal>;
pub type NativeGlobalGetBridgeId = Idx<NativeGlobalGetBridge>;
pub type NativeGlobalSetBridgeId = Idx<NativeGlobalSetBridge>;
pub type NativeGlobalAddressBridgeId = Idx<NativeGlobalAddressBridge>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackFamilyId = Idx<ForeignCallbackFamily>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type ArrayTypeId = Idx<ArrayType>;
pub type DispatchSlotId = Idx<DispatchSlot>;
pub type RootScanId = Idx<RefScan>;
pub type LayoutId = Idx<Layout>;
pub type TypeDescriptorId = Idx<TypeDescriptor>;
pub type ExternalTypeDescriptorId = Idx<ExternalTypeDescriptor>;
pub type ExternalCallableId = Idx<ExternalCallable>;

/// Typed index into `Module::functions`. Functions remain in emission order,
/// while call destinations no longer use their symbols as semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalFunctionId(u32);

impl LocalFunctionId {
    pub const fn from_u32(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_u32(self) -> u32 {
        self.0
    }
}

/// Function-local reference to a complete persistent safepoint identity.
/// The integer is never used to derive the persistent or runtime id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SafepointSiteRef(u32);

impl SafepointSiteRef {
    pub const fn from_u32(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_u32(self) -> u32 {
        self.0
    }
}

/// Compiler-owned scalar domains that are semantically disjoint from every
/// Scoop source integer type.  Codegen currently represents each domain as an
/// LLVM `i64`, but that physical choice must not erase the domain before the
/// final lowering step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarKind {
    /// Byte count passed to allocation/runtime layout primitives.
    ByteSize,
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
            Self::ByteSize => "byte-size",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineSuspendStateId(NonZeroU32);

impl CoroutineSuspendStateId {
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoroutineFrameState {
    Initial,
    Running,
    Completed,
    Suspended(CoroutineSuspendStateId),
    ResumeFailure(CoroutineSuspendStateId),
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

/// A constant from one closed compiler-owned scalar domain.  Its variant is
/// the type witness; a contradictory kind/value pair cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarValue {
    ByteSize(u64),
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
            Self::ByteSize(_) => MachineScalarKind::ByteSize,
            Self::EnumTag(_) => MachineScalarKind::EnumTag,
            Self::InitializationOutcome(_) => MachineScalarKind::InitializationOutcome,
            Self::CoroutineFrameState(_) => MachineScalarKind::CoroutineFrameState,
            Self::CoroutineAdapterState(_) => MachineScalarKind::CoroutineAdapterState,
            Self::ForeignCallbackStatus(_) => MachineScalarKind::ForeignCallbackStatus,
            Self::PointerElementOffset(_) => MachineScalarKind::PointerElementOffset,
        }
    }

    /// Frozen runtime encoding.  Only codegen should normally need this
    /// projection; it never converts the value into a source `Int`/`UInt`.
    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::ByteSize(size) => size,
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

/// A type after layout resolution: maps directly onto LLVM types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirType {
    Void,
    I1,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    MachineScalar(MachineScalarKind),
    Ptr(PointerKind),
    /// A managed object and its exact interface table: `{ AS1, metadata }`.
    Interface,
    /// Opaque Itanium EH landing-pad record (`{ ptr, i32 }` in LLVM).
    /// It is produced by exception pads and may be consumed by `Resume`.
    ExceptionRecord,
    /// Tuple / Unit values: an LLVM literal struct.
    Aggregate(Vec<LirType>),
    /// A named Scoop struct. Its exact physical layout is carried by the
    /// module's `StructDef`, so packed and over-aligned layouts cannot be
    /// erased into an anonymous natural aggregate.
    Struct(StructDefId),
    /// An enum value; the representation is fixed by
    /// `EnumDef::repr` (niche pointer or tagged union, spec 7.4).
    Enum(EnumDefId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// A GC-traced reference to a managed heap object.
    Managed,
    /// A native address that the GC must neither trace nor relocate.
    Raw,
    /// An executable function address.
    Code,
    /// An immortal runtime descriptor or dispatch-table address.
    Metadata,
}

/// Pointer provenance admitted by a null-niche enum representation.
///
/// Metadata pointers are deliberately excluded: they are immortal compiler
/// infrastructure addresses, not source values that may inhabit `Option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NullNicheKind {
    Managed,
    Raw,
    Code,
    Interface,
}

impl NullNicheKind {
    pub const fn storage_type(self) -> LirType {
        match self {
            Self::Managed => MANAGED_PTR,
            Self::Raw => RAW_PTR,
            Self::Code => CODE_PTR,
            Self::Interface => LirType::Interface,
        }
    }

    pub const fn layout(self, profile: LirTargetProfile) -> (u64, u64) {
        let layout = match self {
            Self::Managed => profile.managed_pointer_layout(),
            Self::Raw => profile.data_pointer().layout(),
            Self::Code => profile.code_pointer().layout(),
            Self::Interface => return (16, 8),
        };
        (layout.size_bytes(), layout.alignment_bytes())
    }

    pub const fn dump(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Raw => "raw",
            Self::Code => "code",
            Self::Interface => "interface",
        }
    }
}

pub const MANAGED_PTR: LirType = LirType::Ptr(PointerKind::Managed);
pub const RAW_PTR: LirType = LirType::Ptr(PointerKind::Raw);
pub const CODE_PTR: LirType = LirType::Ptr(PointerKind::Code);
pub const METADATA_PTR: LirType = LirType::Ptr(PointerKind::Metadata);

impl PointerKind {
    pub fn dump(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Raw => "raw",
            Self::Code => "code",
            Self::Metadata => "metadata",
        }
    }
}

impl LirType {
    pub const fn floating(kind: FloatKind) -> Self {
        match kind {
            FloatKind::F32 => Self::F32,
            FloatKind::F64 => Self::F64,
        }
    }

    pub fn dump(&self) -> String {
        match self {
            LirType::Void => "void".to_string(),
            LirType::I1 => "i1".to_string(),
            LirType::I8 => "i8".to_string(),
            LirType::I16 => "i16".to_string(),
            LirType::I32 => "i32".to_string(),
            LirType::I64 => "i64".to_string(),
            LirType::F32 => "f32".to_string(),
            LirType::F64 => "f64".to_string(),
            LirType::MachineScalar(kind) => format!("machine<{}>", kind.name()),
            LirType::Ptr(kind) => format!("ptr<{}>", kind.dump()),
            LirType::Interface => "interface{object,itab}".to_owned(),
            LirType::ExceptionRecord => "exception_record".to_string(),
            LirType::Aggregate(elements) => {
                let inner: Vec<String> = elements.iter().map(LirType::dump).collect();
                format!("{{{}}}", inner.join(", "))
            }
            LirType::Struct(id) => format!("struct{}", id.into_raw()),
            LirType::Enum(id) => format!("enum{}", id.into_raw()),
        }
    }
}
