use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::DispatchDeclarationOwnerV1;
use crate::ids::derive_persistent_id;
use crate::{
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentExactTypeId,
    PersistentFunctionId, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchRoleV1 {
    VirtualMethod,
    InterfaceMethod,
    PropertyGetter,
    PropertySetter,
}

impl WireEncodeV1 for DispatchRoleV1 {
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
pub struct DispatchSlotKeyV1 {
    owner: DispatchDeclarationOwnerV1,
    role: DispatchRoleV1,
}

impl DispatchSlotKeyV1 {
    pub const fn virtual_method(owner: PersistentFunctionId) -> Self {
        Self {
            owner: DispatchDeclarationOwnerV1::Function(owner),
            role: DispatchRoleV1::VirtualMethod,
        }
    }

    pub const fn interface_method(owner: PersistentFunctionId) -> Self {
        Self {
            owner: DispatchDeclarationOwnerV1::Function(owner),
            role: DispatchRoleV1::InterfaceMethod,
        }
    }

    pub const fn property_getter(owner: PersistentPropertyAccessorId) -> Self {
        Self {
            owner: DispatchDeclarationOwnerV1::Accessor(owner),
            role: DispatchRoleV1::PropertyGetter,
        }
    }

    pub const fn property_setter(owner: PersistentPropertyAccessorId) -> Self {
        Self {
            owner: DispatchDeclarationOwnerV1::Accessor(owner),
            role: DispatchRoleV1::PropertySetter,
        }
    }

    pub const fn owner(&self) -> DispatchDeclarationOwnerV1 {
        self.owner
    }

    pub const fn role(&self) -> DispatchRoleV1 {
        self.role
    }
}

impl WireEncodeV1 for DispatchSlotKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentDispatchSlotId {
    pub fn from_key(key: &DispatchSlotKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-dispatch-slot-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchTableRoleV1 {
    VTable,
    ITable,
}

impl WireEncodeV1 for DispatchTableRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::VTable => 1,
            Self::ITable => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalExactInterfaceV1 {
    Absent,
    Present(PersistentExactTypeId),
}

impl WireEncodeV1 for OptionalExactInterfaceV1 {
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
pub struct DispatchTableKeyV1 {
    exact_type: PersistentExactTypeId,
    role: DispatchTableRoleV1,
    interface: OptionalExactInterfaceV1,
}

impl DispatchTableKeyV1 {
    pub const fn vtable(exact_type: PersistentExactTypeId) -> Self {
        Self {
            exact_type,
            role: DispatchTableRoleV1::VTable,
            interface: OptionalExactInterfaceV1::Absent,
        }
    }

    pub const fn itable(
        exact_type: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    ) -> Self {
        Self {
            exact_type,
            role: DispatchTableRoleV1::ITable,
            interface: OptionalExactInterfaceV1::Present(interface),
        }
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn role(&self) -> DispatchTableRoleV1 {
        self.role
    }

    pub const fn interface(&self) -> OptionalExactInterfaceV1 {
        self.interface
    }
}

impl WireEncodeV1 for DispatchTableKeyV1 {
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
    pub fn from_key(key: &DispatchTableKeyV1) -> Result<Self, HashError> {
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
    value: &impl WireEncodeV1,
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
        DispatchRoleV1, DispatchSlotKeyV1, DispatchTableKeyV1, DispatchTableRoleV1,
        OptionalExactInterfaceV1,
    };
    use crate::{
        ConeIdentity, DispatchDeclarationOwnerV1, PersistentDispatchSlotId,
        PersistentDispatchTableId, PersistentExactTypeId, PersistentFunctionId,
        PersistentPropertyAccessorId,
    };

    #[test]
    fn dispatch_slot_has_fixed_identity() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = DispatchSlotKeyV1::virtual_method(function);
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
            DispatchSlotKeyV1::interface_method(function).owner(),
            DispatchDeclarationOwnerV1::Function(function)
        );
        let setter = DispatchSlotKeyV1::property_setter(accessor);
        assert_eq!(
            setter.owner(),
            DispatchDeclarationOwnerV1::Accessor(accessor)
        );
        assert_eq!(setter.role(), DispatchRoleV1::PropertySetter);
    }

    #[test]
    fn vtable_has_explicit_absent_interface_and_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = DispatchTableKeyV1::vtable(exact);
        assert_eq!(key.role(), DispatchTableRoleV1::VTable);
        assert_eq!(key.interface(), OptionalExactInterfaceV1::Absent);
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
        let key = DispatchTableKeyV1::itable(exact, interface);
        assert_eq!(key.role(), DispatchTableRoleV1::ITable);
        assert_eq!(
            key.interface(),
            OptionalExactInterfaceV1::Present(interface)
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
