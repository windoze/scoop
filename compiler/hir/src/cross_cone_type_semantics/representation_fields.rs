//! Kind-specific field records reconstructed from the canonical identity key.

use std::fmt;

use scoop_identity::{
    EnumVariantFieldKey, FieldIdentityError, FieldIdentityKey, FieldIdentityView,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId, PersistentTypeId,
    SignatureTypeKey,
};
use scoop_wire::{Encoder, WireEncode};

use crate::SourceNominalId;

mod decode;
#[cfg(test)]
mod tests;
pub use decode::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructRepresentationFieldV1 {
    field: PersistentFieldId,
    owner: SourceNominalId,
    value_type: SignatureTypeKey,
}

impl StructRepresentationFieldV1 {
    pub fn try_new(
        key: &FieldIdentityKey,
        value_type: SignatureTypeKey,
    ) -> Result<Self, RepresentationFieldBuildError> {
        let FieldIdentityView::SourceDeclared { owner, .. } = key.view() else {
            return Err(RepresentationFieldBuildError::ExpectedStructField);
        };
        let field =
            PersistentFieldId::from_key(key).map_err(RepresentationFieldBuildError::Identity)?;
        Ok(Self {
            field,
            owner,
            value_type,
        })
    }
    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassRepresentationFieldOwnerV1 {
    SourceClass(SourceNominalId),
    ObjectBackingClass(PersistentTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRepresentationFieldV1 {
    field: PersistentFieldId,
    owner: ClassRepresentationFieldOwnerV1,
    value_type: SignatureTypeKey,
}

impl ClassRepresentationFieldV1 {
    pub fn try_new(
        key: &FieldIdentityKey,
        value_type: SignatureTypeKey,
    ) -> Result<Self, RepresentationFieldBuildError> {
        let owner = match key.view() {
            FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => {
                ClassRepresentationFieldOwnerV1::SourceClass(owner)
            }
            FieldIdentityView::Generated { owner, key }
                if key.object_backing_property().is_some() =>
            {
                ClassRepresentationFieldOwnerV1::ObjectBackingClass(owner)
            }
            _ => return Err(RepresentationFieldBuildError::ExpectedClassField),
        };
        let field =
            PersistentFieldId::from_key(key).map_err(RepresentationFieldBuildError::Identity)?;
        Ok(Self {
            field,
            owner,
            value_type,
        })
    }
    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }
    pub const fn owner(&self) -> ClassRepresentationFieldOwnerV1 {
        self.owner
    }
    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumRepresentationFieldV1 {
    field: PersistentEnumVariantFieldId,
    variant: PersistentEnumVariantId,
    value_type: SignatureTypeKey,
}

impl EnumRepresentationFieldV1 {
    pub fn try_new(
        key: &EnumVariantFieldKey,
        value_type: SignatureTypeKey,
    ) -> Result<Self, RepresentationFieldBuildError> {
        let field = PersistentEnumVariantFieldId::from_key(key)
            .map_err(RepresentationFieldBuildError::EnumIdentity)?;
        Ok(Self {
            field,
            variant: key.variant(),
            value_type,
        })
    }
    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }
    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }
    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

macro_rules! field_wire {
    ($field:ty) => {
        impl WireEncode for $field {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.field.encode(encoder)?;
                encoder.field(2)?;
                self.value_type.encode(encoder)
            }
        }
    };
}
field_wire!(StructRepresentationFieldV1);
field_wire!(ClassRepresentationFieldV1);
field_wire!(EnumRepresentationFieldV1);

#[derive(Debug)]
pub enum RepresentationFieldBuildError {
    ExpectedStructField,
    ExpectedClassField,
    Identity(FieldIdentityError),
    EnumIdentity(scoop_wire::HashError),
}

impl fmt::Display for RepresentationFieldBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedStructField => {
                f.write_str("representation requires a declared source struct field")
            }
            Self::ExpectedClassField => f.write_str(
                "representation requires class backing/delegate or object backing storage",
            ),
            Self::Identity(error) => write!(f, "invalid representation field identity: {error}"),
            Self::EnumIdentity(error) => {
                write!(f, "invalid representation enum field identity: {error}")
            }
        }
    }
}
impl std::error::Error for RepresentationFieldBuildError {}
