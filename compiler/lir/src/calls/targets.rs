use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Default)]
pub struct CallTargets {
    pub void_signatures: Arena<VoidCallSignature>,
    pub elided_zst_signatures: Arena<ElidedZstCallSignature>,
    pub direct_signatures: Arena<DirectCallSignature>,
    pub indirect_result_signatures: Arena<IndirectResultCallSignature>,
    pub managed_targets: ProtocolCallTargets<ManagedCallDestination>,
    pub no_gc_targets: ProtocolCallTargets<NoGcCallDestination>,
    pub c_targets: ProtocolCallTargets<CCallDestination>,
    pub native_borrowed_targets: ProtocolCallTargets<NativeBorrowedCallDestination>,
    pub dispatch_slots: DispatchSlots,
    /// Function-local recursive scan programs passed to runtime entries that
    /// receive addressable inline values (currently boxing payloads).
    pub root_scans: Arena<RefScan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoidCallSignature {
    arguments: Vec<AbiArgument>,
    calling_convention: CallingConvention,
}

impl VoidCallSignature {
    pub const fn new(arguments: Vec<AbiArgument>, calling_convention: CallingConvention) -> Self {
        Self {
            arguments,
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElidedZstCallSignature {
    arguments: Vec<AbiArgument>,
    result: AbiZst,
    calling_convention: CallingConvention,
}

impl ElidedZstCallSignature {
    pub const fn new(
        arguments: Vec<AbiArgument>,
        result: AbiZst,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result,
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> &AbiZst {
        &self.result
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCallSignature {
    arguments: Vec<AbiArgument>,
    result: AbiDirectValue,
    calling_convention: CallingConvention,
}

impl DirectCallSignature {
    pub fn new(
        arguments: Vec<AbiArgument>,
        result: impl Into<AbiDirectValue>,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result: result.into(),
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> &AbiDirectValue {
        &self.result
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }
}

/// The physical meaning of the leading storage pointer on an indirect-result
/// call. Only Scoop ABI calls may use `ScoopSret`; a C storage bridge receives
/// an ordinary pointer and must never inherit Scoop `sret` attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndirectResultConvention {
    ScoopSret,
    CStoragePointer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectResultCallSignature {
    arguments: Vec<AbiArgument>,
    result: AbiValue,
    convention: IndirectResultConvention,
    calling_convention: CallingConvention,
}

impl IndirectResultCallSignature {
    pub const fn scoop_sret(
        arguments: Vec<AbiArgument>,
        result: AbiValue,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result,
            convention: IndirectResultConvention::ScoopSret,
            calling_convention,
        }
    }

    pub const fn c_storage_pointer(
        arguments: Vec<AbiArgument>,
        result: AbiValue,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result,
            convention: IndirectResultConvention::CStoragePointer,
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> &AbiValue {
        &self.result
    }

    pub const fn convention(&self) -> IndirectResultConvention {
        self.convention
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }
}

pub type VoidCallSignatureId = Idx<VoidCallSignature>;
pub type ElidedZstCallSignatureId = Idx<ElidedZstCallSignature>;
pub type DirectCallSignatureId = Idx<DirectCallSignature>;
pub type IndirectResultCallSignatureId = Idx<IndirectResultCallSignature>;

/// One protocol- and return-convention-specific call target.  Destination and
/// signature are one atomic entity; a call site can no longer pair a target
/// with a separately selected, potentially incompatible signature.
#[derive(Debug)]
pub struct CallTarget<Destination, Signature> {
    pub destination: Destination,
    pub signature: Signature,
}

pub type VoidCallTargetId<Destination> = Idx<CallTarget<Destination, VoidCallSignatureId>>;
pub type ElidedZstCallTargetId<Destination> =
    Idx<CallTarget<Destination, ElidedZstCallSignatureId>>;
pub type DirectCallTargetId<Destination> = Idx<CallTarget<Destination, DirectCallSignatureId>>;
pub type IndirectResultCallTargetId<Destination> =
    Idx<CallTarget<Destination, IndirectResultCallSignatureId>>;

pub type ManagedVoidTargetId = VoidCallTargetId<ManagedCallDestination>;
pub type ManagedElidedZstTargetId = ElidedZstCallTargetId<ManagedCallDestination>;
pub type ManagedDirectTargetId = DirectCallTargetId<ManagedCallDestination>;
pub type ManagedIndirectResultTargetId = IndirectResultCallTargetId<ManagedCallDestination>;
pub type NoGcVoidTargetId = VoidCallTargetId<NoGcCallDestination>;
pub type NoGcElidedZstTargetId = ElidedZstCallTargetId<NoGcCallDestination>;
pub type NoGcDirectTargetId = DirectCallTargetId<NoGcCallDestination>;
pub type NoGcIndirectResultTargetId = IndirectResultCallTargetId<NoGcCallDestination>;
pub type CVoidTargetId = VoidCallTargetId<CCallDestination>;
pub type CElidedZstTargetId = ElidedZstCallTargetId<CCallDestination>;
pub type CDirectTargetId = DirectCallTargetId<CCallDestination>;
pub type CIndirectResultTargetId = IndirectResultCallTargetId<CCallDestination>;
pub type NativeBorrowedVoidTargetId = VoidCallTargetId<NativeBorrowedCallDestination>;
pub type NativeBorrowedElidedZstTargetId = ElidedZstCallTargetId<NativeBorrowedCallDestination>;
pub type NativeBorrowedDirectTargetId = DirectCallTargetId<NativeBorrowedCallDestination>;
pub type NativeBorrowedIndirectResultTargetId =
    IndirectResultCallTargetId<NativeBorrowedCallDestination>;

#[derive(Debug)]
pub struct ProtocolCallTargets<Destination> {
    pub void: Arena<CallTarget<Destination, VoidCallSignatureId>>,
    pub elided_zst: Arena<CallTarget<Destination, ElidedZstCallSignatureId>>,
    pub direct: Arena<CallTarget<Destination, DirectCallSignatureId>>,
    pub indirect_result: Arena<CallTarget<Destination, IndirectResultCallSignatureId>>,
}

impl<Destination> Default for ProtocolCallTargets<Destination> {
    fn default() -> Self {
        Self {
            void: Arena::new(),
            elided_zst: Arena::new(),
            direct: Arena::new(),
            indirect_result: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDestination {
    Local(LocalFunctionId),
    External(ExternalCallableId),
    Runtime(RuntimeFunction),
    Extern(ExternFunctionId),
    Dispatch { table: Value, slot: DispatchSlotId },
}

/// Effect-refined identities into `Module::functions`. The registry is the
/// sole producer, so a call target cannot attach an independently selected GC
/// effect to an untyped local function id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManagedLocalFunctionRef(LocalFunctionId);

impl ManagedLocalFunctionRef {
    pub fn declaration(self) -> LocalFunctionId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoGcLocalFunctionRef(LocalFunctionId);

impl NoGcLocalFunctionRef {
    pub fn declaration(self) -> LocalFunctionId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalFunctionRef {
    Managed(ManagedLocalFunctionRef),
    NoGc(NoGcLocalFunctionRef),
}

impl LocalFunctionRef {
    pub fn declaration(self) -> LocalFunctionId {
        match self {
            Self::Managed(reference) => reference.declaration(),
            Self::NoGc(reference) => reference.declaration(),
        }
    }
}

/// Declaration-order registry and sole producer of effect-refined local
/// function identities. LIR lowering registers each MIR top-level function
/// exactly once before lowering any body, so recursive calls are typed without
/// placeholders or later reclassification.
#[derive(Debug, Default)]
pub struct LocalFunctionIdentities {
    next: u32,
}

impl LocalFunctionIdentities {
    pub fn alloc_managed(&mut self) -> ManagedLocalFunctionRef {
        ManagedLocalFunctionRef(self.alloc())
    }

    pub fn alloc_no_gc(&mut self) -> NoGcLocalFunctionRef {
        NoGcLocalFunctionRef(self.alloc())
    }

    fn alloc(&mut self) -> LocalFunctionId {
        let id = LocalFunctionId::from_u32(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("the LIR local function identity space is exhausted");
        id
    }
}

/// A destination that can only be reached through a managed statepoint.
/// Each variant carries an identity from the matching producer registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedCallDestination {
    Local(ManagedLocalFunctionRef),
    External(ExternalCallableId),
    Runtime(ManagedRuntimeFunction),
    Dispatch {
        table: Value,
        slot: ManagedDispatchSlotRef,
    },
}

impl ManagedCallDestination {
    pub fn local(function: ManagedLocalFunctionRef) -> Self {
        Self::Local(function)
    }

    pub fn external(function: ExternalCallableId) -> Self {
        Self::External(function)
    }

    pub fn runtime(function: ManagedRuntimeFunction) -> Self {
        Self::Runtime(function)
    }

    pub fn dispatch(table: Value, slot: ManagedDispatchSlotRef) -> Self {
        Self::Dispatch { table, slot }
    }

    pub fn view(self) -> CallDestination {
        match self {
            Self::Local(function) => CallDestination::Local(function.declaration()),
            Self::External(function) => CallDestination::External(function),
            Self::Runtime(function) => CallDestination::Runtime(RuntimeFunction::Managed(function)),
            Self::Dispatch { table, slot } => CallDestination::Dispatch {
                table,
                slot: slot.declaration(),
            },
        }
    }
}

/// A destination statically guaranteed not to safepoint or transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoGcCallDestination {
    Local(NoGcLocalFunctionRef),
    External(ExternalCallableId),
    Runtime(NoGcRuntimeFunction),
    Dispatch {
        table: Value,
        slot: NoGcDispatchSlotRef,
    },
}

impl NoGcCallDestination {
    pub fn local(function: NoGcLocalFunctionRef) -> Self {
        Self::Local(function)
    }

    pub fn external(function: ExternalCallableId) -> Self {
        Self::External(function)
    }

    pub fn runtime(function: NoGcRuntimeFunction) -> Self {
        Self::Runtime(function)
    }

    pub fn dispatch(table: Value, slot: NoGcDispatchSlotRef) -> Self {
        Self::Dispatch { table, slot }
    }

    pub fn view(self) -> CallDestination {
        match self {
            Self::Local(function) => CallDestination::Local(function.declaration()),
            Self::External(function) => CallDestination::External(function),
            Self::Runtime(function) => CallDestination::Runtime(RuntimeFunction::NoGc(function)),
            Self::Dispatch { table, slot } => CallDestination::Dispatch {
                table,
                slot: slot.declaration(),
            },
        }
    }
}

/// Complete C-ABI destinations shared by NativeSafe, GCLeaf and release calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CCallDestination(CExternFunctionRef);

impl CCallDestination {
    pub fn extern_function(function: CExternFunctionRef) -> Self {
        Self(function)
    }

    pub fn view(self) -> CallDestination {
        CallDestination::Extern(self.0.declaration())
    }
}

/// Scoop-ABI outbound calls are the only native-borrowed destinations in LIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBorrowedCallDestination(ScoopExternFunctionRef);

impl NativeBorrowedCallDestination {
    pub fn extern_function(function: ScoopExternFunctionRef) -> Self {
        Self(function)
    }

    pub fn view(self) -> CallDestination {
        CallDestination::Extern(self.0.declaration())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSlot {
    pub kind: DispatchKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedDispatchSlotRef(DispatchSlotId);

impl ManagedDispatchSlotRef {
    pub fn declaration(self) -> DispatchSlotId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoGcDispatchSlotRef(DispatchSlotId);

impl NoGcDispatchSlotRef {
    pub fn declaration(self) -> DispatchSlotId {
        self.0
    }
}

/// Function-local declaration store and sole producer of effect-refined
/// dispatch identities. The physical slot description remains shared because
/// codegen consumes it only after the enclosing destination fixed the effect.
#[derive(Debug, Default)]
pub struct DispatchSlots {
    declarations: Arena<DispatchSlot>,
}

impl DispatchSlots {
    pub fn alloc_managed(&mut self, slot: DispatchSlot) -> ManagedDispatchSlotRef {
        ManagedDispatchSlotRef(self.declarations.alloc(slot))
    }

    pub fn alloc_no_gc(&mut self, slot: DispatchSlot) -> NoGcDispatchSlotRef {
        NoGcDispatchSlotRef(self.declarations.alloc(slot))
    }

    pub fn get(&self, id: DispatchSlotId) -> Option<&DispatchSlot> {
        let index = id.into_raw().into_u32() as usize;
        (index < self.declarations.len()).then(|| &self.declarations[id])
    }
}

impl std::ops::Index<DispatchSlotId> for DispatchSlots {
    type Output = DispatchSlot;

    fn index(&self, index: DispatchSlotId) -> &Self::Output {
        &self.declarations[index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchKind {
    Virtual,
    Interface,
    Closure,
    FunctionBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ManagedRuntimeFunction {
    ContextPush,
    ContextFork,
    ContextEnsureRoot,

    Safepoint,
    Alloc,
    BoxZst,
    BoxValue,
    GcCollect,
    MaterializeException,
    StringConcat,
    InitializationEnter,
    InitializationSucceed,
    InitializationFail,
    InitializationFailure,
    InitializationCycleMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoGcRuntimeFunction {
    ContextTryGet,
    ContextRestore,
    ContextSnapshot,
    ContextCurrent,
    ContextEnter,
    ContextLeave,

    IsInstance,
    ITableLookup,
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
    GcStats,
    StringCompare,
    Trap,
    Throw,
    Rethrow,
    UnboxZst,
    UnboxValue,
    PushRecursiveRegion,
    PopRecursiveRegion,
}

/// Read-only common view used by mechanical dump/codegen logic. Runtime
/// protocol classification is already fixed by the typed target destination;
/// consumers must not reconstruct it from this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuntimeFunction {
    Managed(ManagedRuntimeFunction),
    NoGc(NoGcRuntimeFunction),
}

impl RuntimeFunction {
    pub const fn requires_dedicated_operation(self) -> bool {
        matches!(
            self,
            Self::Managed(ManagedRuntimeFunction::BoxZst | ManagedRuntimeFunction::BoxValue)
                | Self::NoGc(
                    NoGcRuntimeFunction::UnboxZst
                        | NoGcRuntimeFunction::UnboxValue
                        | NoGcRuntimeFunction::PushRecursiveRegion
                        | NoGcRuntimeFunction::PopRecursiveRegion
                )
        )
    }

    pub const fn symbol(self) -> &'static str {
        crate::RuntimeAbiSymbolV1::LirCall(self).logical_symbol()
    }

    pub const fn wire_family_tag(self) -> u64 {
        match self {
            Self::Managed(_) => 1,
            Self::NoGc(_) => 2,
        }
    }

    pub const fn wire_function_tag(self) -> u64 {
        match self {
            Self::Managed(function) => match function {
                ManagedRuntimeFunction::Safepoint => 1,
                ManagedRuntimeFunction::Alloc => 2,
                ManagedRuntimeFunction::GcCollect => 4,
                ManagedRuntimeFunction::MaterializeException => 5,
                ManagedRuntimeFunction::StringConcat => 6,
                ManagedRuntimeFunction::InitializationEnter => 7,
                ManagedRuntimeFunction::InitializationSucceed => 8,
                ManagedRuntimeFunction::InitializationFail => 9,
                ManagedRuntimeFunction::InitializationFailure => 10,
                ManagedRuntimeFunction::InitializationCycleMessage => 11,
                ManagedRuntimeFunction::BoxZst => 12,
                ManagedRuntimeFunction::BoxValue => 13,
                ManagedRuntimeFunction::ContextPush => 14,
                ManagedRuntimeFunction::ContextFork => 15,
                ManagedRuntimeFunction::ContextEnsureRoot => 16,
            },
            Self::NoGc(function) => match function {
                NoGcRuntimeFunction::IsInstance => 1,
                NoGcRuntimeFunction::ITableLookup => 2,
                NoGcRuntimeFunction::Pin => 3,
                NoGcRuntimeFunction::Unpin => 4,
                NoGcRuntimeFunction::GetHandle => 5,
                NoGcRuntimeFunction::ReleaseHandle => 6,
                NoGcRuntimeFunction::GcStats => 7,
                NoGcRuntimeFunction::StringCompare => 8,
                NoGcRuntimeFunction::Trap => 9,
                NoGcRuntimeFunction::Throw => 10,
                NoGcRuntimeFunction::Rethrow => 11,
                NoGcRuntimeFunction::UnboxZst => 12,
                NoGcRuntimeFunction::UnboxValue => 13,
                NoGcRuntimeFunction::PushRecursiveRegion => 14,
                NoGcRuntimeFunction::PopRecursiveRegion => 15,
                NoGcRuntimeFunction::ContextTryGet => 16,
                NoGcRuntimeFunction::ContextRestore => 17,
                NoGcRuntimeFunction::ContextSnapshot => 18,
                NoGcRuntimeFunction::ContextCurrent => 19,
                NoGcRuntimeFunction::ContextEnter => 20,
                NoGcRuntimeFunction::ContextLeave => 21,
            },
        }
    }

    pub const fn from_wire_tags(family: u64, function: u64) -> Option<Self> {
        match (family, function) {
            (1, 1) => Some(Self::Managed(ManagedRuntimeFunction::Safepoint)),
            (1, 2) => Some(Self::Managed(ManagedRuntimeFunction::Alloc)),
            (1, 4) => Some(Self::Managed(ManagedRuntimeFunction::GcCollect)),
            (1, 5) => Some(Self::Managed(ManagedRuntimeFunction::MaterializeException)),
            (1, 6) => Some(Self::Managed(ManagedRuntimeFunction::StringConcat)),
            (1, 7) => Some(Self::Managed(ManagedRuntimeFunction::InitializationEnter)),
            (1, 8) => Some(Self::Managed(ManagedRuntimeFunction::InitializationSucceed)),
            (1, 9) => Some(Self::Managed(ManagedRuntimeFunction::InitializationFail)),
            (1, 10) => Some(Self::Managed(ManagedRuntimeFunction::InitializationFailure)),
            (1, 11) => Some(Self::Managed(
                ManagedRuntimeFunction::InitializationCycleMessage,
            )),
            (1, 12) => Some(Self::Managed(ManagedRuntimeFunction::BoxZst)),
            (1, 13) => Some(Self::Managed(ManagedRuntimeFunction::BoxValue)),
            (1, 14) => Some(Self::Managed(ManagedRuntimeFunction::ContextPush)),
            (1, 15) => Some(Self::Managed(ManagedRuntimeFunction::ContextFork)),
            (1, 16) => Some(Self::Managed(ManagedRuntimeFunction::ContextEnsureRoot)),

            (2, 1) => Some(Self::NoGc(NoGcRuntimeFunction::IsInstance)),
            (2, 2) => Some(Self::NoGc(NoGcRuntimeFunction::ITableLookup)),
            (2, 3) => Some(Self::NoGc(NoGcRuntimeFunction::Pin)),
            (2, 4) => Some(Self::NoGc(NoGcRuntimeFunction::Unpin)),
            (2, 5) => Some(Self::NoGc(NoGcRuntimeFunction::GetHandle)),
            (2, 6) => Some(Self::NoGc(NoGcRuntimeFunction::ReleaseHandle)),
            (2, 7) => Some(Self::NoGc(NoGcRuntimeFunction::GcStats)),
            (2, 8) => Some(Self::NoGc(NoGcRuntimeFunction::StringCompare)),
            (2, 9) => Some(Self::NoGc(NoGcRuntimeFunction::Trap)),
            (2, 10) => Some(Self::NoGc(NoGcRuntimeFunction::Throw)),
            (2, 11) => Some(Self::NoGc(NoGcRuntimeFunction::Rethrow)),
            (2, 12) => Some(Self::NoGc(NoGcRuntimeFunction::UnboxZst)),
            (2, 13) => Some(Self::NoGc(NoGcRuntimeFunction::UnboxValue)),
            (2, 14) => Some(Self::NoGc(NoGcRuntimeFunction::PushRecursiveRegion)),
            (2, 15) => Some(Self::NoGc(NoGcRuntimeFunction::PopRecursiveRegion)),
            (2, 16) => Some(Self::NoGc(NoGcRuntimeFunction::ContextTryGet)),
            (2, 17) => Some(Self::NoGc(NoGcRuntimeFunction::ContextRestore)),
            (2, 18) => Some(Self::NoGc(NoGcRuntimeFunction::ContextSnapshot)),
            (2, 19) => Some(Self::NoGc(NoGcRuntimeFunction::ContextCurrent)),
            (2, 20) => Some(Self::NoGc(NoGcRuntimeFunction::ContextEnter)),
            (2, 21) => Some(Self::NoGc(NoGcRuntimeFunction::ContextLeave)),

            _ => None,
        }
    }
}
