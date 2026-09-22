//! Source-shape witnesses used only to close native boundary validation.

use std::collections::BTreeSet;

use scoop_identity::{
    CLayoutOverride, EnumVariantFieldKey, EnumVariantIdentityError, EnumVariantIdentityKey,
    FieldIdentityError, FieldIdentityKey, NominalDeclarationOwner, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentGenericTypeId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationIdentityError, SourceDeclarationKey,
};
use scoop_wire::{Encoder, HashError, WireEncode};

mod decode;
mod errors;
mod policy;
mod shape;
pub use decode::{
    DecodedNativeBoundaryCLayoutPolicy, DecodedNativeBoundaryFieldDefinition,
    DecodedNativeBoundaryNominalOwner, DecodedNativeBoundaryNominalShape,
    DecodedNativeBoundaryTypeDefinitionRecord, DecodedNativeBoundaryVariantDefinition,
    DecodedNativeBoundaryVariantFieldDefinition, NativeBoundaryResolutionError,
    NativeBoundaryResolver,
};
pub use errors::NativeBoundaryDefinitionError;
pub use shape::{NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryNominalOwner {
    Concrete(PersistentTypeId),
    GenericTemplate(PersistentGenericTypeId),
}

impl NativeBoundaryNominalOwner {
    fn from_declaration(
        declaration: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        match NominalDeclarationOwner::from_source_declaration(declaration)? {
            NominalDeclarationOwner::Concrete(id) => Ok(Self::Concrete(id)),
            NominalDeclarationOwner::GenericTemplate(id) => Ok(Self::GenericTemplate(id)),
        }
    }

    pub const fn declaration_owner(self) -> NominalDeclarationOwner {
        match self {
            Self::Concrete(id) => NominalDeclarationOwner::Concrete(id),
            Self::GenericTemplate(id) => NominalDeclarationOwner::GenericTemplate(id),
        }
    }

    pub const fn kind_tag(self) -> u8 {
        match self {
            Self::Concrete(_) => 1,
            Self::GenericTemplate(_) => 2,
        }
    }

    pub fn raw_id(self) -> [u8; 32] {
        match self {
            Self::Concrete(id) => *id.as_array(),
            Self::GenericTemplate(id) => *id.as_array(),
        }
    }

    pub fn compare_sort_key(self, other: Self) -> std::cmp::Ordering {
        self.kind_tag()
            .cmp(&other.kind_tag())
            .then_with(|| self.raw_id().cmp(&other.raw_id()))
    }
}

impl WireEncode for NativeBoundaryNominalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_value_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryFieldDefinition {
    field: PersistentFieldId,
    owner: NominalDeclarationOwner,
    ty: SignatureTypeKey,
}

impl NativeBoundaryFieldDefinition {
    pub fn new(
        key: &FieldIdentityKey,
        ty: SignatureTypeKey,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = key
            .source_owner()
            .ok_or(NativeBoundaryDefinitionError::ExpectedSourceField)?;
        let field = PersistentFieldId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::FieldIdentity)?;
        Ok(Self { field, owner, ty })
    }

    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }

    pub const fn owner(&self) -> NominalDeclarationOwner {
        self.owner
    }

    pub const fn ty(&self) -> &SignatureTypeKey {
        &self.ty
    }
}

impl WireEncode for NativeBoundaryFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryVariantFieldDefinition {
    field: PersistentEnumVariantFieldId,
    variant: PersistentEnumVariantId,
    ty: SignatureTypeKey,
}

impl NativeBoundaryVariantFieldDefinition {
    pub fn new(
        key: &EnumVariantFieldKey,
        ty: SignatureTypeKey,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let field = PersistentEnumVariantFieldId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::Hash)?;
        Ok(Self {
            field,
            variant: key.variant(),
            ty,
        })
    }

    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn ty(&self) -> &SignatureTypeKey {
        &self.ty
    }
}

impl WireEncode for NativeBoundaryVariantFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryVariantDefinition {
    variant: PersistentEnumVariantId,
    owner: NominalDeclarationOwner,
    fields: Vec<NativeBoundaryVariantFieldDefinition>,
}

impl NativeBoundaryVariantDefinition {
    pub fn new(
        key: &EnumVariantIdentityKey,
        fields: Vec<NativeBoundaryVariantFieldDefinition>,
    ) -> Result<Self, NativeBoundaryDefinitionError> {
        let owner = key
            .source_owner()
            .ok_or(NativeBoundaryDefinitionError::ExpectedSourceVariant)?;
        let variant = PersistentEnumVariantId::from_key(key)
            .map_err(NativeBoundaryDefinitionError::VariantIdentity)?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if field.variant != variant {
                return Err(NativeBoundaryDefinitionError::VariantFieldOwnerMismatch);
            }
            if !seen.insert(field.field) {
                return Err(NativeBoundaryDefinitionError::DuplicateVariantField);
            }
        }
        Ok(Self {
            variant,
            owner,
            fields,
        })
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn owner(&self) -> NominalDeclarationOwner {
        self.owner
    }

    pub fn fields(&self) -> &[NativeBoundaryVariantFieldDefinition] {
        &self.fields
    }
}

impl WireEncode for NativeBoundaryVariantDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryCLayoutPolicy {
    NotCLayout,
    CLayout {
        aligned: CLayoutOverride,
        packed: CLayoutOverride,
    },
}

impl WireEncode for NativeBoundaryCLayoutPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCLayout => encode_empty_sum(encoder, 1),
            Self::CLayout { aligned, packed } => encode_two_value_sum(encoder, 2, aligned, packed),
        }
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
