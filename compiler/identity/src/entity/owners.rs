use scoop_wire::{Encoder, WireEncode};

use crate::{
    PersistentCallableApplicationId, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
};

use super::{SourceDeclarationIdentityError, SourceDeclarationKey};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyOwner {
    Property(PersistentPropertyId),
    ExtensionProperty(crate::PersistentExtensionPropertyId),
}

impl WireEncode for PropertyOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Property(id) => encode_id_sum(encoder, 1, id),
            Self::ExtensionProperty(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalDeclarationOwner {
    Concrete(PersistentTypeId),
    GenericTemplate(PersistentGenericTypeId),
}

impl NominalDeclarationOwner {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        if key.duplicate_signature().type_parameter_count() == 0 {
            PersistentTypeId::from_source_declaration(key).map(Self::Concrete)
        } else {
            PersistentGenericTypeId::from_source_declaration(key).map(Self::GenericTemplate)
        }
    }
}

impl WireEncode for NominalDeclarationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_id_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalOwner {
    Declaration(NominalDeclarationOwner),
    ExactApplication(crate::PersistentExactTypeId),
}

impl WireEncode for NominalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Declaration(owner) => encode_id_sum(encoder, 1, owner),
            Self::ExactApplication(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableOwner {
    Function(PersistentFunctionId),
    GenericTemplate(PersistentGenericFunctionId),
    Application(PersistentCallableApplicationId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl WireEncode for CallableOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_id_sum(encoder, 2, id),
            Self::Application(id) => encode_id_sum(encoder, 3, id),
            Self::Constructor(id) => encode_id_sum(encoder, 4, id),
            Self::Accessor(id) => encode_id_sum(encoder, 5, id),
            Self::Generated(id) => encode_id_sum(encoder, 6, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchDeclarationOwner {
    Function(PersistentFunctionId),
    Accessor(PersistentPropertyAccessorId),
}

impl WireEncode for DispatchDeclarationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::Accessor(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

fn encode_id_sum(
    encoder: &mut Encoder,
    tag: u64,
    id: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    id.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::PropertyOwner;
    use crate::{ConeIdentity, PersistentPropertyId};

    #[test]
    fn typed_owner_sum_keeps_its_kind_tag() {
        let property = PersistentPropertyId(ConeIdentity::CORE.0);
        assert_eq!(
            encode(&PropertyOwner::Property(property)).unwrap(),
            [b"\xa2\x00\x01\x01\x58\x20".as_slice(), property.as_array()].concat()
        );
    }
}
