use super::*;

#[derive(Debug)]
pub enum TypedCall<Destination> {
    Void {
        target: VoidCallTargetId<Destination>,
        args: Vec<AbiCallArgument>,
    },
    ElidedZst {
        target: ElidedZstCallTargetId<Destination>,
        out: TempId,
        args: Vec<AbiCallArgument>,
    },
    Direct {
        target: DirectCallTargetId<Destination>,
        out: TempId,
        args: Vec<AbiCallArgument>,
    },
    IndirectResult {
        target: IndirectResultCallTargetId<Destination>,
        storage: LocalId,
        args: Vec<AbiCallArgument>,
    },
}

pub type ManagedTypedCall = TypedCall<ManagedCallDestination>;
pub type NoGcTypedCall = TypedCall<NoGcCallDestination>;
pub type CTypedCall = TypedCall<CCallDestination>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedCallResult {
    Void,
    ElidedZst(TempId),
    Direct(TempId),
    IndirectResult(LocalId),
}

impl<Destination> TypedCall<Destination> {
    pub fn args(&self) -> &[AbiCallArgument] {
        match self {
            Self::Void { args, .. }
            | Self::ElidedZst { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match *self {
            Self::Direct { out, .. } => Some(out),
            Self::Void { .. } | Self::ElidedZst { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_temp(&self) -> Option<TempId> {
        match *self {
            Self::ElidedZst { out, .. } | Self::Direct { out, .. } => Some(out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_storage(&self) -> Option<LocalId> {
        match *self {
            Self::IndirectResult { storage, .. } => Some(storage),
            Self::Void { .. } | Self::ElidedZst { .. } | Self::Direct { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match *self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::ElidedZst { out, .. } => TypedCallResult::ElidedZst(out),
            Self::Direct { out, .. } => TypedCallResult::Direct(out),
            Self::IndirectResult { storage, .. } => TypedCallResult::IndirectResult(storage),
        }
    }
}
