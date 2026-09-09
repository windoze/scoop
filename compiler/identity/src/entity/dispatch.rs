use scoop_wire::{Encoder, HashError, WireEncode};

use super::DispatchDeclarationOwner;
use crate::ids::derive_persistent_id;
use crate::{
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentExactTypeId,
    PersistentFunctionId, PersistentPropertyAccessorId,
};

mod decode;

pub use decode::{
    DecodedDispatchSlotKey, DecodedDispatchTableKey, DecodedOptionalExactInterface,
    DispatchIdentityResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchRole {
    VirtualMethod,
    InterfaceMethod,
    PropertyGetter,
    PropertySetter,
}

impl WireEncode for DispatchRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::VirtualMethod => 1,
            Self::InterfaceMethod => 2,
            Self::PropertyGetter => 3,
            Self::PropertySetter => 4,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DispatchSlotKey {
    owner: DispatchDeclarationOwner,
    role: DispatchRole,
}

impl DispatchSlotKey {
    pub const fn virtual_method(owner: PersistentFunctionId) -> Self {
        Self {
            owner: DispatchDeclarationOwner::Function(owner),
            role: DispatchRole::VirtualMethod,
        }
    }

    pub const fn interface_method(owner: PersistentFunctionId) -> Self {
        Self {
            owner: DispatchDeclarationOwner::Function(owner),
            role: DispatchRole::InterfaceMethod,
        }
    }

    pub const fn property_getter(owner: PersistentPropertyAccessorId) -> Self {
        Self {
            owner: DispatchDeclarationOwner::Accessor(owner),
            role: DispatchRole::PropertyGetter,
        }
    }

    pub const fn property_setter(owner: PersistentPropertyAccessorId) -> Self {
        Self {
            owner: DispatchDeclarationOwner::Accessor(owner),
            role: DispatchRole::PropertySetter,
        }
    }

    pub const fn owner(&self) -> DispatchDeclarationOwner {
        self.owner
    }

    pub const fn role(&self) -> DispatchRole {
        self.role
    }
}

impl WireEncode for DispatchSlotKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentDispatchSlotId {
    pub fn from_key(key: &DispatchSlotKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-dispatch-slot-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchTableRole {
    VTable,
    ITable,
}

impl WireEncode for DispatchTableRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::VTable => 1,
            Self::ITable => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalExactInterface {
    Absent,
    Present(PersistentExactTypeId),
}

impl WireEncode for OptionalExactInterface {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Present(interface) => encode_value_sum(encoder, 2, interface),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DispatchTableKey {
    exact_type: PersistentExactTypeId,
    role: DispatchTableRole,
    interface: OptionalExactInterface,
}

impl DispatchTableKey {
    pub const fn vtable(exact_type: PersistentExactTypeId) -> Self {
        Self {
            exact_type,
            role: DispatchTableRole::VTable,
            interface: OptionalExactInterface::Absent,
        }
    }

    pub const fn itable(
        exact_type: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    ) -> Self {
        Self {
            exact_type,
            role: DispatchTableRole::ITable,
            interface: OptionalExactInterface::Present(interface),
        }
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn role(&self) -> DispatchTableRole {
        self.role
    }

    pub const fn interface(&self) -> OptionalExactInterface {
        self.interface
    }
}

impl WireEncode for DispatchTableKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.interface.encode(encoder)
    }
}

impl PersistentDispatchTableId {
    pub fn from_key(key: &DispatchTableKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-dispatch-table-id-v1", key)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        DispatchRole, DispatchSlotKey, DispatchTableKey, DispatchTableRole, OptionalExactInterface,
    };
    use crate::{
        ConeIdentity, DispatchDeclarationOwner, PersistentDispatchSlotId,
        PersistentDispatchTableId, PersistentExactTypeId, PersistentFunctionId,
        PersistentPropertyAccessorId,
    };

    #[test]
    fn dispatch_slot_has_fixed_identity() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = DispatchSlotKey::virtual_method(function);
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a201a20001015820{function}0201")
        );
        assert_eq!(
            PersistentDispatchSlotId::from_key(&key)
                .unwrap()
                .to_string(),
            "e111d6166e7608c8b82218057ceefc06ff069636938b3e689d677ad401fbe36a"
        );
    }

    #[test]
    fn slot_constructors_close_the_owner_role_matrix() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let accessor = PersistentPropertyAccessorId(ConeIdentity::SINGLE_FILE.0);
        assert_eq!(
            DispatchSlotKey::interface_method(function).owner(),
            DispatchDeclarationOwner::Function(function)
        );
        let setter = DispatchSlotKey::property_setter(accessor);
        assert_eq!(setter.owner(), DispatchDeclarationOwner::Accessor(accessor));
        assert_eq!(setter.role(), DispatchRole::PropertySetter);
    }

    #[test]
    fn vtable_has_explicit_absent_interface_and_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = DispatchTableKey::vtable(exact);
        assert_eq!(key.role(), DispatchTableRole::VTable);
        assert_eq!(key.interface(), OptionalExactInterface::Absent);
        assert_eq!(
            PersistentDispatchTableId::from_key(&key)
                .unwrap()
                .to_string(),
            "e6916e02a80b2fade456dc2abe2930ac8e617b3c451ecddfc1f74cfa15cc7d46"
        );
    }

    #[test]
    fn itable_requires_an_interface_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let interface = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let key = DispatchTableKey::itable(exact, interface);
        assert_eq!(key.role(), DispatchTableRole::ITable);
        assert_eq!(key.interface(), OptionalExactInterface::Present(interface));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
