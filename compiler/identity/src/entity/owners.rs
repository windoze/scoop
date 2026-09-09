use scoop_wire::{Encoder, WireEncodeV1};

use crate::{
    PersistentCallableApplicationId, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
};

use super::{SourceDeclarationIdentityError, SourceDeclarationKeyV1};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyOwnerV1 {
    Property(PersistentPropertyId),
    ExtensionProperty(crate::PersistentExtensionPropertyId),
}

impl WireEncodeV1 for PropertyOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Property(id) => encode_id_sum(encoder, 1, id),
            Self::ExtensionProperty(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalDeclarationOwnerV1 {
    Concrete(PersistentTypeId),
    GenericTemplate(PersistentGenericTypeId),
}

impl NominalDeclarationOwnerV1 {
    pub fn from_source_declaration(
        key: &SourceDeclarationKeyV1,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        if key.duplicate_signature().type_parameter_count() == 0 {
            PersistentTypeId::from_source_declaration(key).map(Self::Concrete)
        } else {
            PersistentGenericTypeId::from_source_declaration(key).map(Self::GenericTemplate)
        }
    }
}

impl WireEncodeV1 for NominalDeclarationOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_id_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalOwnerV1 {
    Declaration(NominalDeclarationOwnerV1),
    ExactApplication(crate::PersistentExactTypeId),
}

impl WireEncodeV1 for NominalOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Declaration(owner) => encode_id_sum(encoder, 1, owner),
            Self::ExactApplication(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableOwnerV1 {
    Function(PersistentFunctionId),
    GenericTemplate(PersistentGenericFunctionId),
    Application(PersistentCallableApplicationId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl WireEncodeV1 for CallableOwnerV1 {
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
pub enum DispatchDeclarationOwnerV1 {
    Function(PersistentFunctionId),
    Accessor(PersistentPropertyAccessorId),
}

impl WireEncodeV1 for DispatchDeclarationOwnerV1 {
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
    id: &impl WireEncodeV1,
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

    use super::PropertyOwnerV1;
    use crate::{ConeIdentity, PersistentPropertyId};

    #[test]
    fn typed_owner_sum_keeps_its_kind_tag() {
        let property = PersistentPropertyId(ConeIdentity::CORE.0);
        assert_eq!(
            encode(&PropertyOwnerV1::Property(property)).unwrap(),
            [b"\xa2\x00\x01\x01\x58\x20".as_slice(), property.as_array()].concat()
        );
    }
}
