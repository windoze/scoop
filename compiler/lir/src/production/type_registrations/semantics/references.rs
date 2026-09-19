use crate::RuntimeFunction;
use scoop_identity::{PersistentCallableBodyId, PersistentExactTypeId};

/// Typed origin of a descriptor pointer stored inside a local TypeDescriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDescriptorRefV1 {
    Local(PersistentExactTypeId),
    CoreExternal(PersistentExactTypeId),
}

impl StrongTypeDescriptorRefV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Local(exact_type) | Self::CoreExternal(exact_type) => exact_type,
        }
    }
}

/// Typed semantic target of one vtable or itable slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDispatchCallableRefV1 {
    Local(PersistentCallableBodyId),
    CoreExternal(PersistentCallableBodyId),
    Runtime(RuntimeFunction),
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::StrongTypeDescriptorRefV1 {}
    impl Sealed for crate::StrongTypeDescriptorRefV2 {}
}

/// Closed descriptor-reference family used by versioned registration products.
pub trait StrongDescriptorReference: sealed::Sealed + Copy + scoop_wire::WireEncode {
    fn exact_type(self) -> PersistentExactTypeId;
    fn encode_optional(
        reference: Option<Self>,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError>;
}

impl StrongDescriptorReference for StrongTypeDescriptorRefV1 {
    fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type()
    }

    fn encode_optional(
        reference: Option<Self>,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        use crate::OptionalStrongTypeDescriptorRefV2 as Optional;
        use scoop_wire::WireEncode;
        match reference {
            None => Optional::Absent,
            Some(Self::Local(exact)) => Optional::Local(exact),
            Some(Self::CoreExternal(exact)) => Optional::CoreExternal(exact),
        }
        .encode(encoder)
    }
}

impl StrongDescriptorReference for crate::StrongTypeDescriptorRefV2 {
    fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type()
    }

    fn encode_optional(
        reference: Option<Self>,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        use crate::OptionalStrongTypeDescriptorRefV2 as Optional;
        use scoop_wire::WireEncode;
        match reference {
            None => Optional::Absent,
            Some(Self::Local(exact)) => Optional::Local(exact),
            Some(Self::CoreExternal(exact)) => Optional::CoreExternal(exact),
            Some(Self::DependencyExternal { provider, exact }) => {
                Optional::DependencyExternal { provider, exact }
            }
        }
        .encode(encoder)
    }
}

impl scoop_wire::WireEncode for StrongTypeDescriptorRefV1 {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        use crate::StrongTypeDescriptorRefV2 as Ref;
        match self {
            Self::Local(exact) => Ref::Local(*exact),
            Self::CoreExternal(exact) => Ref::CoreExternal(*exact),
        }
        .encode(encoder)
    }
}

impl scoop_wire::WireEncode for StrongTypeDispatchCallableRefV1 {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        use crate::StrongTypeDispatchCallableRefV2 as Ref;
        match self {
            Self::Local(body) => Ref::Local(*body),
            Self::CoreExternal(body) => Ref::CoreExternal(*body),
            Self::Runtime(function) => Ref::Runtime(*function),
        }
        .encode(encoder)
    }
}
