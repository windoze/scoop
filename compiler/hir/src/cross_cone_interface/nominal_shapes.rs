use std::collections::BTreeSet;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentIdResolver, PersistentObjectValueId,
    SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{PublicNominalKindV1, SignatureTypeReferenceResolver};
use crate::{IntrinsicTypeTarget, NominalCLayoutPolicyV1, NominalIntrinsicRepresentationV1};

mod enumeration;
mod errors;
mod metered_resolution;
mod semantics;
mod wire;

pub use enumeration::{
    DecodedEnumSourceFieldV1, DecodedEnumSourceVariantV1, EnumSourceFieldV1, EnumSourceShapeV1,
    EnumSourceVariantStyleV1, EnumSourceVariantV1,
};
pub use errors::{
    EnumSourceFieldResolutionError, EnumSourceVariantBuildError, EnumSourceVariantResolutionError,
    NominalSourceShapeBuildError, NominalSourceShapeResolutionError,
    StructSourceFieldResolutionError,
};
pub use semantics::{
    EnumSourceFieldSelectorV1, EnumSourceFieldSemanticError, EnumSourceVariantSemanticError,
    NominalSourceShapeSemanticAuthority, NominalSourceShapeSemanticError,
    ObjectSourceShapeSemanticError, StructSourceFieldSemanticError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructSourceFieldV1 {
    field: PersistentFieldId,
    value_type: SignatureTypeKey,
}

impl StructSourceFieldV1 {
    pub const fn new(field: PersistentFieldId, value_type: SignatureTypeKey) -> Self {
        Self { field, value_type }
    }

    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

impl WireEncode for StructSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedStructSourceFieldV1 {
    field: DecodedPersistentId<PersistentFieldId>,
    value_type: DecodedSignatureTypeKey,
}

impl DecodedStructSourceFieldV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<StructSourceFieldV1, StructSourceFieldResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let field = resolver
            .resolve(self.field)
            .map_err(StructSourceFieldResolutionError::Field)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(StructSourceFieldResolutionError::ValueType)?;
        Ok(StructSourceFieldV1 { field, value_type })
    }
}

impl WireEncode for DecodedStructSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedStructSourceFieldV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructSourceShapeV1 {
    fields: Vec<StructSourceFieldV1>,
    c_layout_policy: NominalCLayoutPolicyV1,
}

impl StructSourceShapeV1 {
    pub fn try_new(
        fields: Vec<StructSourceFieldV1>,
        c_layout_policy: NominalCLayoutPolicyV1,
    ) -> Result<Self, NominalSourceShapeBuildError> {
        if fields.is_empty() && matches!(c_layout_policy, NominalCLayoutPolicyV1::CLayout { .. }) {
            return Err(NominalSourceShapeBuildError::EmptyCLayout);
        }
        u32::try_from(fields.len())
            .map_err(|_| NominalSourceShapeBuildError::TooManyStructFields)?;
        let mut ids = BTreeSet::new();
        for field in &fields {
            if !ids.insert(field.field()) {
                return Err(NominalSourceShapeBuildError::DuplicateStructField(
                    field.field(),
                ));
            }
        }
        Ok(Self {
            fields,
            c_layout_policy,
        })
    }

    pub const fn c_layout_policy(&self) -> NominalCLayoutPolicyV1 {
        self.c_layout_policy
    }

    pub fn fields(&self) -> &[StructSourceFieldV1] {
        &self.fields
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectSourceShapeV1 {
    value: PersistentObjectValueId,
}

impl ObjectSourceShapeV1 {
    pub const fn new(value: PersistentObjectValueId) -> Self {
        Self { value }
    }

    pub const fn value(self) -> PersistentObjectValueId {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalSourceShapeV1 {
    Class,
    Interface,
    Struct(StructSourceShapeV1),
    Enum(EnumSourceShapeV1),
    Object(ObjectSourceShapeV1),
    Intrinsic(NominalIntrinsicRepresentationV1),
}

impl NominalSourceShapeV1 {
    pub const fn kind(&self) -> PublicNominalKindV1 {
        match self {
            Self::Class => PublicNominalKindV1::Class,
            Self::Interface => PublicNominalKindV1::Interface,
            Self::Struct(_) => PublicNominalKindV1::Struct,
            Self::Enum(_) => PublicNominalKindV1::Enum,
            Self::Object(_) => PublicNominalKindV1::Object,
            Self::Intrinsic(representation) => match representation.family().target() {
                IntrinsicTypeTarget::Struct => PublicNominalKindV1::Struct,
                IntrinsicTypeTarget::Class => PublicNominalKindV1::Class,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNominalSourceShapeV1 {
    Class,
    Interface,
    Struct {
        fields: Vec<DecodedStructSourceFieldV1>,
        c_layout_policy: NominalCLayoutPolicyV1,
    },
    Enum(Vec<DecodedEnumSourceVariantV1>),
    Object(DecodedPersistentId<PersistentObjectValueId>),
    Intrinsic(NominalIntrinsicRepresentationV1),
}

impl DecodedNominalSourceShapeV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSourceShapeV1, NominalSourceShapeResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        match self {
            Self::Class => Ok(NominalSourceShapeV1::Class),
            Self::Interface => Ok(NominalSourceShapeV1::Interface),
            Self::Intrinsic(representation) => Ok(NominalSourceShapeV1::Intrinsic(representation)),
            Self::Struct {
                fields,
                c_layout_policy,
            } => resolve_struct_shape(fields, c_layout_policy, resolver)
                .map(NominalSourceShapeV1::Struct),
            Self::Enum(variants) => {
                resolve_enum_shape(variants, resolver).map(NominalSourceShapeV1::Enum)
            }
            Self::Object(value) => resolver
                .resolve(value)
                .map(ObjectSourceShapeV1::new)
                .map(NominalSourceShapeV1::Object)
                .map_err(NominalSourceShapeResolutionError::ObjectValue),
        }
    }
}

pub trait NominalSourceShapeResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentFieldId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
    + PersistentIdResolver<PersistentObjectValueId, Error = E>
{
}

impl<R, E> NominalSourceShapeResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentFieldId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
        + PersistentIdResolver<PersistentObjectValueId, Error = E>
{
}

fn resolve_struct_shape<R, E>(
    decoded: Vec<DecodedStructSourceFieldV1>,
    c_layout_policy: NominalCLayoutPolicyV1,
    resolver: &mut R,
) -> Result<StructSourceShapeV1, NominalSourceShapeResolutionError<E>>
where
    R: NominalSourceShapeResolver<E>,
{
    if decoded.is_empty() && matches!(c_layout_policy, NominalCLayoutPolicyV1::CLayout { .. }) {
        return Err(NominalSourceShapeResolutionError::EmptyCLayout);
    }
    u32::try_from(decoded.len())
        .map_err(|_| NominalSourceShapeResolutionError::TooManyStructFields)?;
    let mut fields = Vec::with_capacity(decoded.len());
    let mut ids = BTreeSet::new();
    for (index, field) in decoded.into_iter().enumerate() {
        let field = field
            .resolve(resolver)
            .map_err(|error| NominalSourceShapeResolutionError::StructField { index, error })?;
        if !ids.insert(field.field()) {
            return Err(NominalSourceShapeResolutionError::DuplicateStructField {
                index,
                field: field.field(),
            });
        }
        fields.push(field);
    }
    Ok(StructSourceShapeV1 {
        fields,
        c_layout_policy,
    })
}

fn resolve_enum_shape<R, E>(
    decoded: Vec<DecodedEnumSourceVariantV1>,
    resolver: &mut R,
) -> Result<EnumSourceShapeV1, NominalSourceShapeResolutionError<E>>
where
    R: NominalSourceShapeResolver<E>,
{
    u32::try_from(decoded.len())
        .map_err(|_| NominalSourceShapeResolutionError::TooManyEnumVariants)?;
    let mut variants = Vec::with_capacity(decoded.len());
    let mut ids = BTreeSet::new();
    for (index, variant) in decoded.into_iter().enumerate() {
        let variant = variant
            .resolve(resolver)
            .map_err(|error| NominalSourceShapeResolutionError::EnumVariant { index, error })?;
        if !ids.insert(variant.variant()) {
            return Err(NominalSourceShapeResolutionError::DuplicateEnumVariant {
                index,
                variant: variant.variant(),
            });
        }
        variants.push(variant);
    }
    Ok(EnumSourceShapeV1 { variants })
}

fn validate_variant_fields(
    style: EnumSourceVariantStyleV1,
    fields: &[EnumSourceFieldV1],
) -> Result<(), EnumSourceVariantBuildError> {
    u32::try_from(fields.len()).map_err(|_| EnumSourceVariantBuildError::TooManyFields)?;
    if style == EnumSourceVariantStyleV1::Unit && !fields.is_empty() {
        return Err(EnumSourceVariantBuildError::UnitFields);
    }
    let mut ids = BTreeSet::new();
    for field in fields {
        if !ids.insert(field.field()) {
            return Err(EnumSourceVariantBuildError::DuplicateField(field.field()));
        }
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
