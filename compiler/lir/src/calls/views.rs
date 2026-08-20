use super::*;

/// Protocol-neutral read-only projection used by dump and codegen after the
/// enclosing callsite arm has already selected the protocol-specific arena.
pub enum TypedCallView<'a> {
    Void {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a VoidCallSignature,
        args: &'a [AbiCallArgument],
    },
    ElidedZst {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a ElidedZstCallSignature,
        out: TempId,
        args: &'a [AbiCallArgument],
    },
    Direct {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a DirectCallSignature,
        out: TempId,
        args: &'a [AbiCallArgument],
    },
    IndirectResult {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a IndirectResultCallSignature,
        storage: LocalId,
        args: &'a [AbiCallArgument],
    },
}

impl TypedCallView<'_> {
    pub fn target_raw(&self) -> u32 {
        match self {
            Self::Void { target, .. }
            | Self::ElidedZst { target, .. }
            | Self::Direct { target, .. }
            | Self::IndirectResult { target, .. } => *target,
        }
    }

    pub const fn return_convention_name(&self) -> &'static str {
        match self {
            Self::Void { .. } => "void",
            Self::ElidedZst { .. } => "elided-zst",
            Self::Direct { .. } => "direct",
            Self::IndirectResult { .. } => "indirect",
        }
    }

    pub fn destination(&self) -> CallDestination {
        match self {
            Self::Void { destination, .. }
            | Self::ElidedZst { destination, .. }
            | Self::Direct { destination, .. }
            | Self::IndirectResult { destination, .. } => *destination,
        }
    }

    pub fn args(&self) -> &[AbiCallArgument] {
        match self {
            Self::Void { args, .. }
            | Self::ElidedZst { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        match self {
            Self::Void { signature, .. } => signature.arguments(),
            Self::ElidedZst { signature, .. } => signature.arguments(),
            Self::Direct { signature, .. } => signature.arguments(),
            Self::IndirectResult { signature, .. } => signature.arguments(),
        }
    }

    pub fn result_scan(&self) -> &RefScan {
        match self {
            Self::Void { .. } => &RefScan::None,
            Self::ElidedZst { signature, .. } => signature.result().scan(),
            Self::Direct { signature, .. } => signature.result().scan(),
            Self::IndirectResult { signature, .. } => signature.result().scan(),
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::Direct { out, .. } => Some(*out),
            Self::Void { .. } | Self::ElidedZst { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_temp(&self) -> Option<TempId> {
        match self {
            Self::ElidedZst { out, .. } | Self::Direct { out, .. } => Some(*out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::ElidedZst { out, .. } => TypedCallResult::ElidedZst(*out),
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
                self.void_call_view(*target, args, targets, destination_view)
            }
            TypedCall::ElidedZst { target, out, args } => {
                self.elided_zst_call_view(*target, *out, args, targets, destination_view)
            }
            TypedCall::Direct { target, out, args } => {
                self.direct_call_view(*target, *out, args, targets, destination_view)
            }
            TypedCall::IndirectResult {
                target,
                storage,
                args,
            } => self.indirect_result_call_view(*target, *storage, args, targets, destination_view),
        }
    }

    pub(super) fn void_call_view<'a, Destination: Copy>(
        &'a self,
        target: VoidCallTargetId<Destination>,
        args: &'a [AbiCallArgument],
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        let target_value = &targets.void[target];
        TypedCallView::Void {
            target: target.into_raw().into_u32(),
            signature_id: target_value.signature.into_raw().into_u32(),
            destination: destination_view(target_value.destination),
            signature: &self.void_signatures[target_value.signature],
            args,
        }
    }

    pub(super) fn elided_zst_call_view<'a, Destination: Copy>(
        &'a self,
        target: ElidedZstCallTargetId<Destination>,
        out: TempId,
        args: &'a [AbiCallArgument],
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        let target_value = &targets.elided_zst[target];
        TypedCallView::ElidedZst {
            target: target.into_raw().into_u32(),
            signature_id: target_value.signature.into_raw().into_u32(),
            destination: destination_view(target_value.destination),
            signature: &self.elided_zst_signatures[target_value.signature],
            out,
            args,
        }
    }

    pub(super) fn direct_call_view<'a, Destination: Copy>(
        &'a self,
        target: DirectCallTargetId<Destination>,
        out: TempId,
        args: &'a [AbiCallArgument],
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        let target_value = &targets.direct[target];
        TypedCallView::Direct {
            target: target.into_raw().into_u32(),
            signature_id: target_value.signature.into_raw().into_u32(),
            destination: destination_view(target_value.destination),
            signature: &self.direct_signatures[target_value.signature],
            out,
            args,
        }
    }

    pub(super) fn indirect_result_call_view<'a, Destination: Copy>(
        &'a self,
        target: IndirectResultCallTargetId<Destination>,
        storage: LocalId,
        args: &'a [AbiCallArgument],
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        let target_value = &targets.indirect_result[target];
        TypedCallView::IndirectResult {
            target: target.into_raw().into_u32(),
            signature_id: target_value.signature.into_raw().into_u32(),
            destination: destination_view(target_value.destination),
            signature: &self.indirect_result_signatures[target_value.signature],
            storage,
            args,
        }
    }
}
