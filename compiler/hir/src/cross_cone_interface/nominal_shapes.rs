use std::collections::BTreeSet;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentIdResolver, PersistentObjectValueId,
    SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{PublicNominalKindV1, SignatureTypeReferenceResolver};

mod errors;
mod metered_resolution;
mod semantics;

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
}

impl StructSourceShapeV1 {
    pub fn try_new(fields: Vec<StructSourceFieldV1>) -> Result<Self, NominalSourceShapeBuildError> {
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
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &[StructSourceFieldV1] {
        &self.fields
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnumSourceVariantStyleV1 {
    Unit,
    Positional,
    Named,
    Constructor,
}

impl WireEncode for EnumSourceVariantStyleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Unit => 1,
            Self::Positional => 2,
            Self::Named => 3,
            Self::Constructor => 4,
        })
    }
}

impl WireDecode for EnumSourceVariantStyleV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Unit),
            2 => Ok(Self::Positional),
            3 => Ok(Self::Named),
            4 => Ok(Self::Constructor),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceFieldV1 {
    field: PersistentEnumVariantFieldId,
    value_type: SignatureTypeKey,
}

impl EnumSourceFieldV1 {
    pub const fn new(field: PersistentEnumVariantFieldId, value_type: SignatureTypeKey) -> Self {
        Self { field, value_type }
    }

    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

impl WireEncode for EnumSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumSourceFieldV1 {
    field: DecodedPersistentId<PersistentEnumVariantFieldId>,
    value_type: DecodedSignatureTypeKey,
}

impl DecodedEnumSourceFieldV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumSourceFieldV1, EnumSourceFieldResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let field = resolver
            .resolve(self.field)
            .map_err(EnumSourceFieldResolutionError::Field)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(EnumSourceFieldResolutionError::ValueType)?;
        Ok(EnumSourceFieldV1 { field, value_type })
    }
}

impl WireEncode for DecodedEnumSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedEnumSourceFieldV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceVariantV1 {
    variant: PersistentEnumVariantId,
    style: EnumSourceVariantStyleV1,
    fields: Vec<EnumSourceFieldV1>,
}

impl EnumSourceVariantV1 {
    pub fn try_new(
        variant: PersistentEnumVariantId,
        style: EnumSourceVariantStyleV1,
        fields: Vec<EnumSourceFieldV1>,
    ) -> Result<Self, EnumSourceVariantBuildError> {
        validate_variant_fields(style, &fields)?;
        Ok(Self {
            variant,
            style,
            fields,
        })
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn style(&self) -> EnumSourceVariantStyleV1 {
        self.style
    }

    pub fn fields(&self) -> &[EnumSourceFieldV1] {
        &self.fields
    }
}

impl WireEncode for EnumSourceVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.style.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumSourceVariantV1 {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    style: EnumSourceVariantStyleV1,
    fields: Vec<DecodedEnumSourceFieldV1>,
}

impl DecodedEnumSourceVariantV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumSourceVariantV1, EnumSourceVariantResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let variant = resolver
            .resolve(self.variant)
            .map_err(EnumSourceVariantResolutionError::Variant)?;
        u32::try_from(self.fields.len())
            .map_err(|_| EnumSourceVariantResolutionError::TooManyFields)?;
        if self.style == EnumSourceVariantStyleV1::Unit && !self.fields.is_empty() {
            return Err(EnumSourceVariantResolutionError::UnitFields);
        }
        let mut fields = Vec::with_capacity(self.fields.len());
        let mut ids = BTreeSet::new();
        for (index, field) in self.fields.into_iter().enumerate() {
            let field = field
                .resolve(resolver)
                .map_err(|error| EnumSourceVariantResolutionError::Field { index, error })?;
            if !ids.insert(field.field()) {
                return Err(EnumSourceVariantResolutionError::DuplicateField {
                    index,
                    field: field.field(),
                });
            }
            fields.push(field);
        }
        Ok(EnumSourceVariantV1 {
            variant,
            style: self.style,
            fields,
        })
    }
}

impl WireEncode for DecodedEnumSourceVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.style.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.fields)
    }
}

impl WireDecode for DecodedEnumSourceVariantV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            style: decoder.field(2, EnumSourceVariantStyleV1::decode)?,
            fields: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedEnumSourceFieldV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceShapeV1 {
    variants: Vec<EnumSourceVariantV1>,
}

impl EnumSourceShapeV1 {
    pub fn try_new(
        variants: Vec<EnumSourceVariantV1>,
    ) -> Result<Self, NominalSourceShapeBuildError> {
        u32::try_from(variants.len())
            .map_err(|_| NominalSourceShapeBuildError::TooManyEnumVariants)?;
        let mut ids = BTreeSet::new();
        for variant in &variants {
            if !ids.insert(variant.variant()) {
                return Err(NominalSourceShapeBuildError::DuplicateEnumVariant(
                    variant.variant(),
                ));
            }
        }
        Ok(Self { variants })
    }

    pub fn variants(&self) -> &[EnumSourceVariantV1] {
        &self.variants
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
}

impl NominalSourceShapeV1 {
    pub const fn kind(&self) -> PublicNominalKindV1 {
        match self {
            Self::Class => PublicNominalKindV1::Class,
            Self::Interface => PublicNominalKindV1::Interface,
            Self::Struct(_) => PublicNominalKindV1::Struct,
            Self::Enum(_) => PublicNominalKindV1::Enum,
            Self::Object(_) => PublicNominalKindV1::Object,
        }
    }
}

impl WireEncode for NominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class => encode_empty_sum(encoder, 1),
            Self::Interface => encode_empty_sum(encoder, 2),
            Self::Struct(shape) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, &shape.fields)
            }
            Self::Enum(shape) => {
                encoder.map(2)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encode_sequence(encoder, &shape.variants)
            }
            Self::Object(shape) => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                shape.value.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNominalSourceShapeV1 {
    Class,
    Interface,
    Struct(Vec<DecodedStructSourceFieldV1>),
    Enum(Vec<DecodedEnumSourceVariantV1>),
    Object(DecodedPersistentId<PersistentObjectValueId>),
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
            Self::Struct(fields) => {
                resolve_struct_shape(fields, resolver).map(NominalSourceShapeV1::Struct)
            }
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

impl WireEncode for DecodedNominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class => encode_empty_sum(encoder, 1),
            Self::Interface => encode_empty_sum(encoder, 2),
            Self::Struct(fields) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, fields)
            }
            Self::Enum(variants) => {
                encoder.map(2)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
            Self::Object(value) => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedNominalSourceShapeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Class)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Interface)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder
                            .decode_array(|decoder, _| DecodedStructSourceFieldV1::decode(decoder))
                    })
                    .map(Self::Struct)
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder
                            .decode_array(|decoder, _| DecodedEnumSourceVariantV1::decode(decoder))
                    })
                    .map(Self::Enum)
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Object)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
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
    resolver: &mut R,
) -> Result<StructSourceShapeV1, NominalSourceShapeResolutionError<E>>
where
    R: NominalSourceShapeResolver<E>,
{
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
    Ok(StructSourceShapeV1 { fields })
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
