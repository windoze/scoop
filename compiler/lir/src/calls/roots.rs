use super::*;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeBorrowedRootSet(Vec<CallerRoot>);

impl NativeBorrowedRootSet {
    pub fn new(roots: Vec<CallerRoot>) -> Self {
        Self(roots)
    }

    pub fn as_slice(&self) -> &[CallerRoot] {
        &self.0
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
    pub fn args(&self) -> &[AbiCallArgument] {
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

    pub fn result_temp(&self) -> Option<TempId> {
        match self {
            Self::Managed(site) => site.call.result_temp(),
            Self::NoGc(site) => site.call.result_temp(),
            Self::NativeSafe(site) => site.call.result_temp(),
            Self::NativeBorrowed(site) => site.call.result_temp(),
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
            Self::NativeBorrowed(site) => site.call.view(targets).call.destination(),
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
    pub fn args(&self) -> &[AbiCallArgument] {
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

    pub fn result_temp(&self) -> Option<TempId> {
        match self {
            Self::Managed(site) => site.call.result_temp(),
            Self::NoGc(site) => site.call.result_temp(),
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

    pub(crate) fn dump(self) -> String {
        match self {
            Self::Param(index) => format!("param{index}"),
            Self::Local(id) => format!("local{}", id.into_raw()),
            Self::Temp(id) => format!("t{}", id.into_raw()),
        }
    }
}
