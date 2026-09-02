use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Default)]
pub struct CallTargets {
    pub void_signatures: Arena<VoidCallSignature>,
    pub direct_signatures: Arena<DirectCallSignature>,
    pub indirect_result_signatures: Arena<IndirectResultCallSignature>,
    pub managed_targets: ProtocolCallTargets<ManagedCallDestination>,
    pub no_gc_targets: ProtocolCallTargets<NoGcCallDestination>,
    pub native_safe_targets: ProtocolCallTargets<NativeSafeCallDestination>,
    pub native_borrowed_targets: ProtocolCallTargets<NativeBorrowedCallDestination>,
    pub dispatch_slots: Arena<DispatchSlot>,
    /// Function-local recursive scan programs passed to runtime entries that
    /// receive addressable inline values (currently boxing payloads).
    pub root_scans: Arena<RefScan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoidCallSignature {
    pub params: Vec<LirType>,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCallSignature {
    pub params: Vec<LirType>,
    pub result: LirType,
    /// Recursive scan of a direct result. Native transitions use it to
    /// publish result storage before re-entering managed code.
    pub result_scan: RefScan,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectResultCallSignature {
    pub params: Vec<LirType>,
    pub result: ResultStorage,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultStorage {
    pub ty: LirType,
    pub scan: RefScan,
}

pub type VoidCallSignatureId = Idx<VoidCallSignature>;
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
pub type DirectCallTargetId<Destination> = Idx<CallTarget<Destination, DirectCallSignatureId>>;
pub type IndirectResultCallTargetId<Destination> =
    Idx<CallTarget<Destination, IndirectResultCallSignatureId>>;

pub type ManagedVoidTargetId = VoidCallTargetId<ManagedCallDestination>;
pub type ManagedDirectTargetId = DirectCallTargetId<ManagedCallDestination>;
pub type ManagedIndirectResultTargetId = IndirectResultCallTargetId<ManagedCallDestination>;
pub type NoGcVoidTargetId = VoidCallTargetId<NoGcCallDestination>;
pub type NoGcDirectTargetId = DirectCallTargetId<NoGcCallDestination>;
pub type NoGcIndirectResultTargetId = IndirectResultCallTargetId<NoGcCallDestination>;
pub type NativeSafeVoidTargetId = VoidCallTargetId<NativeSafeCallDestination>;
pub type NativeSafeDirectTargetId = DirectCallTargetId<NativeSafeCallDestination>;
pub type NativeSafeIndirectResultTargetId = IndirectResultCallTargetId<NativeSafeCallDestination>;
pub type NativeBorrowedVoidTargetId = VoidCallTargetId<NativeBorrowedCallDestination>;
pub type NativeBorrowedDirectTargetId = DirectCallTargetId<NativeBorrowedCallDestination>;
pub type NativeBorrowedIndirectResultTargetId =
    IndirectResultCallTargetId<NativeBorrowedCallDestination>;

#[derive(Debug)]
pub struct ProtocolCallTargets<Destination> {
    pub void: Arena<CallTarget<Destination, VoidCallSignatureId>>,
    pub direct: Arena<CallTarget<Destination, DirectCallSignatureId>>,
    pub indirect_result: Arena<CallTarget<Destination, IndirectResultCallSignatureId>>,
}

impl<Destination> Default for ProtocolCallTargets<Destination> {
    fn default() -> Self {
        Self {
            void: Arena::new(),
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

/// A destination that can only be reached through a managed statepoint.
/// The inner common representation is exposed only as a read-only view;
/// construction is closed over the legal destination categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedCallDestination(CallDestination);

impl ManagedCallDestination {
    pub fn local(function: LocalFunctionId) -> Self {
        Self(CallDestination::Local(function))
    }

    pub fn runtime(function: ManagedRuntimeFunction) -> Self {
        Self(CallDestination::Runtime(RuntimeFunction::Managed(function)))
    }

    pub fn dispatch(table: Value, slot: DispatchSlotId) -> Self {
        Self(CallDestination::Dispatch { table, slot })
    }

    pub fn view(self) -> CallDestination {
        self.0
    }

    pub fn from_view(destination: CallDestination) -> Option<Self> {
        matches!(
            destination,
            CallDestination::Local(_)
                | CallDestination::Runtime(RuntimeFunction::Managed(_))
                | CallDestination::Dispatch { .. }
        )
        .then_some(Self(destination))
    }
}

/// A destination statically guaranteed not to safepoint or transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoGcCallDestination(CallDestination);

impl NoGcCallDestination {
    pub fn local(function: LocalFunctionId) -> Self {
        Self(CallDestination::Local(function))
    }

    pub fn runtime(function: NoGcRuntimeFunction) -> Self {
        Self(CallDestination::Runtime(RuntimeFunction::NoGc(function)))
    }

    pub fn dispatch(table: Value, slot: DispatchSlotId) -> Self {
        Self(CallDestination::Dispatch { table, slot })
    }

    pub fn view(self) -> CallDestination {
        self.0
    }

    pub fn from_view(destination: CallDestination) -> Option<Self> {
        matches!(
            destination,
            CallDestination::Local(_)
                | CallDestination::Runtime(RuntimeFunction::NoGc(_))
                | CallDestination::Dispatch { .. }
        )
        .then_some(Self(destination))
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
pub enum DispatchKind {
    Virtual,
    Interface,
    Closure,
    FunctionBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedRuntimeFunction {
    Safepoint,
    Alloc,
    Box,
    GcCollect,
    MaterializeException,
    StringConcat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoGcRuntimeFunction {
    IsInstance,
    ITableLookup,
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
    GcStats,
    Trap,
    Throw,
    Rethrow,
}

/// Read-only common view used by mechanical dump/codegen logic. Runtime
/// protocol classification is already fixed by the typed target destination;
/// consumers must not reconstruct it from this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFunction {
    Managed(ManagedRuntimeFunction),
    NoGc(NoGcRuntimeFunction),
}

impl RuntimeFunction {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Managed(ManagedRuntimeFunction::Safepoint) => "scoop_rt_safepoint",
            Self::Managed(ManagedRuntimeFunction::Alloc) => "scoop_rt_alloc",
            Self::Managed(ManagedRuntimeFunction::Box) => "scoop_rt_box",
            Self::Managed(ManagedRuntimeFunction::GcCollect) => "scoop_rt_gc_collect",
            Self::Managed(ManagedRuntimeFunction::MaterializeException) => {
                "scoop_rt_materialize_exception"
            }
            Self::Managed(ManagedRuntimeFunction::StringConcat) => "scoop_rt_string_concat",
            Self::NoGc(NoGcRuntimeFunction::IsInstance) => "scoop_rt_is_instance",
            Self::NoGc(NoGcRuntimeFunction::ITableLookup) => "scoop_rt_itable_lookup",
            Self::NoGc(NoGcRuntimeFunction::Pin) => "scoop_rt_pin",
            Self::NoGc(NoGcRuntimeFunction::Unpin) => "scoop_rt_unpin",
            Self::NoGc(NoGcRuntimeFunction::GetHandle) => "scoop_rt_get_handle",
            Self::NoGc(NoGcRuntimeFunction::ReleaseHandle) => "scoop_rt_release_handle",
            Self::NoGc(NoGcRuntimeFunction::GcStats) => "scoop_rt_gc_stats",
            Self::NoGc(NoGcRuntimeFunction::Trap) => "scoop_rt_trap",
            Self::NoGc(NoGcRuntimeFunction::Throw) => "scoop_rt_throw",
            Self::NoGc(NoGcRuntimeFunction::Rethrow) => "scoop_rt_rethrow",
        }
    }
}

#[derive(Debug)]
pub enum TypedCall<Destination> {
    Void {
        target: VoidCallTargetId<Destination>,
        args: Vec<Value>,
    },
    Direct {
        target: DirectCallTargetId<Destination>,
        out: TempId,
        args: Vec<Value>,
    },
    IndirectResult {
        target: IndirectResultCallTargetId<Destination>,
        storage: LocalId,
        args: Vec<Value>,
    },
}

pub type ManagedTypedCall = TypedCall<ManagedCallDestination>;
pub type NoGcTypedCall = TypedCall<NoGcCallDestination>;
pub type NativeSafeTypedCall = TypedCall<NativeSafeCallDestination>;
pub type NativeBorrowedTypedCall = TypedCall<NativeBorrowedCallDestination>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedCallResult {
    Void,
    Direct(TempId),
    IndirectResult(LocalId),
}

impl<Destination> TypedCall<Destination> {
    pub fn args(&self) -> &[Value] {
        match self {
            Self::Void { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match *self {
            Self::Direct { out, .. } => Some(out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_storage(&self) -> Option<LocalId> {
        match *self {
            Self::IndirectResult { storage, .. } => Some(storage),
            Self::Void { .. } | Self::Direct { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match *self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::Direct { out, .. } => TypedCallResult::Direct(out),
            Self::IndirectResult { storage, .. } => TypedCallResult::IndirectResult(storage),
        }
    }
}

/// Protocol-neutral read-only projection used by dump and codegen after the
/// enclosing callsite arm has already selected the protocol-specific arena.
pub enum TypedCallView<'a> {
    Void {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a VoidCallSignature,
        args: &'a [Value],
    },
    Direct {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a DirectCallSignature,
        out: TempId,
        args: &'a [Value],
    },
    IndirectResult {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a IndirectResultCallSignature,
        storage: LocalId,
        args: &'a [Value],
    },
}

impl TypedCallView<'_> {
    pub fn target_raw(&self) -> u32 {
        match self {
            Self::Void { target, .. }
            | Self::Direct { target, .. }
            | Self::IndirectResult { target, .. } => *target,
        }
    }

    pub const fn return_convention_name(&self) -> &'static str {
        match self {
            Self::Void { .. } => "void",
            Self::Direct { .. } => "direct",
            Self::IndirectResult { .. } => "indirect",
        }
    }

    pub fn destination(&self) -> CallDestination {
        match self {
            Self::Void { destination, .. }
            | Self::Direct { destination, .. }
            | Self::IndirectResult { destination, .. } => *destination,
        }
    }

    pub fn args(&self) -> &[Value] {
        match self {
            Self::Void { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn result_scan(&self) -> &RefScan {
        match self {
            Self::Void { .. } => &RefScan::None,
            Self::Direct { signature, .. } => &signature.result_scan,
            Self::IndirectResult { signature, .. } => &signature.result.scan,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::Direct { out, .. } => Some(*out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::Direct { out, .. } => TypedCallResult::Direct(*out),
            Self::IndirectResult { storage, .. } => TypedCallResult::IndirectResult(*storage),
        }
    }
}

impl CallTargets {
    pub fn typed_call_view<'a, Destination: Copy>(
        &'a self,
        call: &'a TypedCall<Destination>,
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        match call {
            TypedCall::Void { target, args } => {
                let target_value = &targets.void[*target];
                TypedCallView::Void {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.void_signatures[target_value.signature],
                    args,
                }
            }
            TypedCall::Direct { target, out, args } => {
                let target_value = &targets.direct[*target];
                TypedCallView::Direct {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.direct_signatures[target_value.signature],
                    out: *out,
                    args,
                }
            }
            TypedCall::IndirectResult {
                target,
                storage,
                args,
            } => {
                let target_value = &targets.indirect_result[*target];
                TypedCallView::IndirectResult {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.indirect_result_signatures[target_value.signature],
                    storage: *storage,
                    args,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManagedLeafPath {
    pub byte_offset: u64,
}

/// A non-empty, flattened, sorted and deduplicated set of managed leaves in
/// one concrete source value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedLeafPaths(Vec<ManagedLeafPath>);

impl ManagedLeafPaths {
    pub fn new(paths: Vec<ManagedLeafPath>) -> Option<Self> {
        let strictly_sorted = paths
            .windows(2)
            .all(|pair| pair[0].byte_offset < pair[1].byte_offset);
        (!paths.is_empty() && strictly_sorted).then_some(Self(paths))
    }

    pub fn as_slice(&self) -> &[ManagedLeafPath] {
        &self.0
    }
}

/// One source whose managed leaves must be present at a statepoint. The set
/// includes values live after the site and movable call operands: LLVM treats
/// direct AS1 operands as roots even if their source dies at the call, while
/// aggregate operands require explicit leaf exposure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatepointLiveValue {
    pub source: CallerRootSource,
    pub ty: LirType,
    pub leaves: ManagedLeafPaths,
}

/// The complete, source-ordered root set for one ordinary statepoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatepointLiveSet(Vec<StatepointLiveValue>);

impl StatepointLiveSet {
    pub fn new(values: Vec<StatepointLiveValue>) -> Option<Self> {
        let strictly_sorted = values
            .windows(2)
            .all(|pair| pair[0].source.sort_key() < pair[1].source.sort_key());
        strictly_sorted.then_some(Self(values))
    }

    pub fn as_slice(&self) -> &[StatepointLiveValue] {
        &self.0
    }
}

/// A value whose address is published in a compiler caller-root frame.
/// Constants and globals cannot appear here: constants have no storage and
/// managed globals are already roots in their own right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallerRootSource {
    Param(u32),
    Local(LocalId),
    Temp(TempId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerRoot {
    pub source: CallerRootSource,
    /// Non-empty recursive scan program relative to the source's storage.
    pub scan: NonEmptyRefScan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionalRoot {
    pub root: CallerRoot,
    /// The pre-invoke value is used from the normal successor. A direct
    /// result is defined by the invoke and cannot be a pre-invoke root.
    pub normal_live: bool,
    /// The pre-invoke value is used from the unwind successor.
    pub unwind_live: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExceptionalRootSet(Vec<ExceptionalRoot>);

impl ExceptionalRootSet {
    pub fn new(roots: Vec<ExceptionalRoot>) -> Self {
        Self(roots)
    }

    pub fn as_slice(&self) -> &[ExceptionalRoot] {
        &self.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeSafeRootSet(Vec<CallerRoot>);

impl NativeSafeRootSet {
    pub fn new(roots: Vec<CallerRoot>) -> Self {
        Self(roots)
    }

    pub fn as_slice(&self) -> &[CallerRoot] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBorrowedResultRoot {
    GcFree,
    Rooted {
        storage: LocalId,
        scan: NonEmptyRefScan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBorrowedRootSet {
    roots: Vec<CallerRoot>,
    pub result: NativeBorrowedResultRoot,
}

impl NativeBorrowedRootSet {
    pub fn new(roots: Vec<CallerRoot>, result: NativeBorrowedResultRoot) -> Self {
        Self { roots, result }
    }

    pub fn as_slice(&self) -> &[CallerRoot] {
        &self.roots
    }
}

#[derive(Debug)]
pub struct ManagedPollSite {
    pub target: ManagedVoidTargetId,
    pub safepoint: SafepointId,
    pub live: StatepointLiveSet,
}

#[derive(Debug)]
pub struct ManagedCallSite {
    pub call: ManagedTypedCall,
    pub safepoint: SafepointId,
    pub live: StatepointLiveSet,
}

#[derive(Debug)]
pub struct NoGcCallSite {
    pub call: NoGcTypedCall,
}

#[derive(Debug)]
pub struct NativeSafeCallSite {
    pub call: NativeSafeTypedCall,
    pub safepoint: SafepointId,
    pub roots: NativeSafeRootSet,
}

#[derive(Debug)]
pub struct NativeBorrowedCallSite {
    pub call: NativeBorrowedTypedCall,
    pub safepoint: SafepointId,
    pub roots: NativeBorrowedRootSet,
}

#[derive(Debug)]
pub enum CallSite {
    Managed(ManagedCallSite),
    NoGc(NoGcCallSite),
    NativeSafe(NativeSafeCallSite),
    NativeBorrowed(NativeBorrowedCallSite),
}

impl CallSite {
    pub fn args(&self) -> &[Value] {
        match self {
            Self::Managed(site) => site.call.args(),
            Self::NoGc(site) => site.call.args(),
            Self::NativeSafe(site) => site.call.args(),
            Self::NativeBorrowed(site) => site.call.args(),
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::Managed(site) => site.call.direct_out(),
            Self::NoGc(site) => site.call.direct_out(),
            Self::NativeSafe(site) => site.call.direct_out(),
            Self::NativeBorrowed(site) => site.call.direct_out(),
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Managed(site) => site.call.result(),
            Self::NoGc(site) => site.call.result(),
            Self::NativeSafe(site) => site.call.result(),
            Self::NativeBorrowed(site) => site.call.result(),
        }
    }

    pub fn destination(&self, targets: &CallTargets) -> CallDestination {
        match self {
            Self::Managed(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    ManagedCallDestination::view,
                )
                .destination(),
            Self::NoGc(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    NoGcCallDestination::view,
                )
                .destination(),
            Self::NativeSafe(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.native_safe_targets,
                    NativeSafeCallDestination::view,
                )
                .destination(),
            Self::NativeBorrowed(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.native_borrowed_targets,
                    NativeBorrowedCallDestination::view,
                )
                .destination(),
        }
    }
}

#[derive(Debug)]
pub struct ManagedInvokeSite {
    pub call: ManagedTypedCall,
    pub safepoint: SafepointId,
    pub roots: ExceptionalRootSet,
    pub normal: BlockId,
    pub unwind: BlockId,
}

#[derive(Debug)]
pub struct NoGcInvokeSite {
    pub call: NoGcTypedCall,
    pub normal: BlockId,
    pub unwind: BlockId,
}

#[derive(Debug)]
pub enum InvokeSite {
    Managed(ManagedInvokeSite),
    NoGc(NoGcInvokeSite),
}

impl InvokeSite {
    pub fn args(&self) -> &[Value] {
        match self {
            Self::Managed(site) => site.call.args(),
            Self::NoGc(site) => site.call.args(),
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::Managed(site) => site.call.direct_out(),
            Self::NoGc(site) => site.call.direct_out(),
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Managed(site) => site.call.result(),
            Self::NoGc(site) => site.call.result(),
        }
    }

    pub fn destination(&self, targets: &CallTargets) -> CallDestination {
        match self {
            Self::Managed(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    ManagedCallDestination::view,
                )
                .destination(),
            Self::NoGc(site) => targets
                .typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    NoGcCallDestination::view,
                )
                .destination(),
        }
    }

    pub fn normal(&self) -> BlockId {
        match self {
            Self::Managed(site) => site.normal,
            Self::NoGc(site) => site.normal,
        }
    }

    pub fn unwind(&self) -> BlockId {
        match self {
            Self::Managed(site) => site.unwind,
            Self::NoGc(site) => site.unwind,
        }
    }
}

impl CallerRootSource {
    fn sort_key(self) -> (u8, u32) {
        match self {
            Self::Param(index) => (0, index),
            Self::Local(id) => (1, id.into_raw().into_u32()),
            Self::Temp(id) => (2, id.into_raw().into_u32()),
        }
    }

    pub(super) fn dump(self) -> String {
        match self {
            Self::Param(index) => format!("param{index}"),
            Self::Local(id) => format!("local{}", id.into_raw()),
            Self::Temp(id) => format!("t{}", id.into_raw()),
        }
    }
}
