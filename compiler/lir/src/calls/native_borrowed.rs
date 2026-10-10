use super::*;

/// An outbound Scoop call and its result publication, classified together by
/// lowering. Indirect results use one storage identity for both purposes.
#[derive(Debug)]
pub enum NativeBorrowedTypedCall {
    Void {
        target: VoidCallTargetId<NativeBorrowedCallDestination>,
        args: Vec<AbiCallArgument>,
    },
    ElidedZst {
        target: ElidedZstCallTargetId<NativeBorrowedCallDestination>,
        out: TempId,
        args: Vec<AbiCallArgument>,
    },
    DirectGcFree {
        target: DirectCallTargetId<NativeBorrowedCallDestination>,
        out: TempId,
        args: Vec<AbiCallArgument>,
    },
    DirectRooted {
        target: DirectCallTargetId<NativeBorrowedCallDestination>,
        out: TempId,
        args: Vec<AbiCallArgument>,
        storage: LocalId,
        scan: NonEmptyRefScan,
    },
    IndirectResultGcFree {
        target: IndirectResultCallTargetId<NativeBorrowedCallDestination>,
        storage: LocalId,
        args: Vec<AbiCallArgument>,
    },
    IndirectResultRooted {
        target: IndirectResultCallTargetId<NativeBorrowedCallDestination>,
        storage: LocalId,
        args: Vec<AbiCallArgument>,
        scan: NonEmptyRefScan,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum NativeBorrowedResultPublication<'a> {
    Void,
    ElidedZst,
    DirectGcFree,
    DirectRooted {
        storage: LocalId,
        scan: &'a NonEmptyRefScan,
    },
    IndirectResultGcFree,
    IndirectResultRooted {
        storage: LocalId,
        scan: &'a NonEmptyRefScan,
    },
}

pub struct NativeBorrowedTypedCallView<'a> {
    pub call: TypedCallView<'a>,
    pub result: NativeBorrowedResultPublication<'a>,
}

impl NativeBorrowedTypedCall {
    pub fn args(&self) -> &[AbiCallArgument] {
        match self {
            Self::Void { args, .. }
            | Self::ElidedZst { args, .. }
            | Self::DirectGcFree { args, .. }
            | Self::DirectRooted { args, .. }
            | Self::IndirectResultGcFree { args, .. }
            | Self::IndirectResultRooted { args, .. } => args,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::DirectGcFree { out, .. } | Self::DirectRooted { out, .. } => Some(*out),
            Self::Void { .. }
            | Self::ElidedZst { .. }
            | Self::IndirectResultGcFree { .. }
            | Self::IndirectResultRooted { .. } => None,
        }
    }

    pub fn result_temp(&self) -> Option<TempId> {
        match self {
            Self::ElidedZst { out, .. }
            | Self::DirectGcFree { out, .. }
            | Self::DirectRooted { out, .. } => Some(*out),
            Self::Void { .. }
            | Self::IndirectResultGcFree { .. }
            | Self::IndirectResultRooted { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::ElidedZst { out, .. } => TypedCallResult::ElidedZst(*out),
            Self::DirectGcFree { out, .. } | Self::DirectRooted { out, .. } => {
                TypedCallResult::Direct(*out)
            }
            Self::IndirectResultGcFree { storage, .. }
            | Self::IndirectResultRooted { storage, .. } => {
                TypedCallResult::IndirectResult(*storage)
            }
        }
    }

    pub fn view<'a>(&'a self, targets: &'a CallTargets) -> NativeBorrowedTypedCallView<'a> {
        let protocol = &targets.native_borrowed_targets;
        let destination = NativeBorrowedCallDestination::view;
        let call = match self {
            Self::Void { target, args } => {
                targets.void_call_view(*target, args, protocol, destination)
            }
            Self::ElidedZst { target, out, args } => {
                targets.elided_zst_call_view(*target, *out, args, protocol, destination)
            }
            Self::DirectGcFree { target, out, args }
            | Self::DirectRooted {
                target, out, args, ..
            } => targets.direct_call_view(*target, *out, args, protocol, destination),
            Self::IndirectResultGcFree {
                target,
                storage,
                args,
            }
            | Self::IndirectResultRooted {
                target,
                storage,
                args,
                ..
            } => targets.indirect_result_call_view(*target, *storage, args, protocol, destination),
        };
        let result = match self {
            Self::Void { .. } => NativeBorrowedResultPublication::Void,
            Self::ElidedZst { .. } => NativeBorrowedResultPublication::ElidedZst,
            Self::DirectGcFree { .. } => NativeBorrowedResultPublication::DirectGcFree,
            Self::DirectRooted { storage, scan, .. } => {
                NativeBorrowedResultPublication::DirectRooted {
                    storage: *storage,
                    scan,
                }
            }
            Self::IndirectResultGcFree { .. } => {
                NativeBorrowedResultPublication::IndirectResultGcFree
            }
            Self::IndirectResultRooted { storage, scan, .. } => {
                NativeBorrowedResultPublication::IndirectResultRooted {
                    storage: *storage,
                    scan,
                }
            }
        };
        NativeBorrowedTypedCallView { call, result }
    }
}
