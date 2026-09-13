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
    pub native_safe_targets: ProtocolCallTargets<NativeSafeCallDestination>,
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
    result: AbiValue,
    calling_convention: CallingConvention,
}

impl DirectCallSignature {
    pub const fn new(
        arguments: Vec<AbiArgument>,
        result: AbiValue,
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

    pub const fn result(&self) -> &AbiValue {
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
pub type NativeSafeVoidTargetId = VoidCallTargetId<NativeSafeCallDestination>;
pub type NativeSafeElidedZstTargetId = ElidedZstCallTargetId<NativeSafeCallDestination>;
pub type NativeSafeDirectTargetId = DirectCallTargetId<NativeSafeCallDestination>;
pub type NativeSafeIndirectResultTargetId = IndirectResultCallTargetId<NativeSafeCallDestination>;
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

    pub fn runtime(function: ManagedRuntimeFunction) -> Self {
        Self::Runtime(function)
    }

    pub fn dispatch(table: Value, slot: ManagedDispatchSlotRef) -> Self {
        Self::Dispatch { table, slot }
    }

    pub fn view(self) -> CallDestination {
        match self {
            Self::Local(function) => CallDestination::Local(function.declaration()),
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

    pub fn runtime(function: NoGcRuntimeFunction) -> Self {
        Self::Runtime(function)
    }

    pub fn dispatch(table: Value, slot: NoGcDispatchSlotRef) -> Self {
        Self::Dispatch { table, slot }
    }

    pub fn view(self) -> CallDestination {
        match self {
            Self::Local(function) => CallDestination::Local(function.declaration()),
            Self::Runtime(function) => CallDestination::Runtime(RuntimeFunction::NoGc(function)),
            Self::Dispatch { table, slot } => CallDestination::Dispatch {
                table,
                slot: slot.declaration(),
            },
        }
    }
}

/// C-ABI outbound calls are the only native-safe destinations in LIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSafeCallDestination(CExternFunctionRef);

impl NativeSafeCallDestination {
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
    Safepoint,
    Alloc,
    Box,
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
    pub const fn symbol(self) -> &'static str {
        crate::RuntimeAbiSymbolV1::LirCall(self).logical_symbol()
    }
}
