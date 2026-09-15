use std::fmt;
use std::sync::Arc;

use scoop_identity::{
    DecodedCLayoutOverride, DecodedPersistentId, DecodedSignatureTypeKey, EnumVariantFieldKey,
    EnumVariantIdentityKey, FieldIdentityKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentKeyResolver, PersistentTypeId, SourceDeclarationKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    ExternalCoreNativeBoundaryDefinitionError, NativeBoundaryCLayoutPolicy,
    NativeBoundaryDefinitionError, NativeBoundaryFieldDefinition, NativeBoundaryNominalOwner,
    NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
    NativeBoundaryVariantDefinition, NativeBoundaryVariantFieldDefinition, encode_empty_sum,
    encode_sequence, encode_tag, encode_two_value_sum, encode_value_sum,
};

pub trait NativeBoundaryResolver<E>:
    PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentFieldId, FieldIdentityKey, Error = E>
    + PersistentKeyResolver<PersistentEnumVariantId, EnumVariantIdentityKey, Error = E>
    + PersistentKeyResolver<PersistentEnumVariantFieldId, EnumVariantFieldKey, Error = E>
{
    fn native_boundary_type_parameter_count(
        &mut self,
        declaration: &SourceDeclarationKey,
    ) -> Result<u32, E>;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNativeBoundaryNominalOwner {
    Concrete(DecodedPersistentId<PersistentTypeId>),
    GenericTemplate(DecodedPersistentId<PersistentGenericTypeId>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedOwnerKind {
    Concrete,
    GenericTemplate,
}

impl DecodedNativeBoundaryNominalOwner {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<(DecodedOwnerKind, Arc<SourceDeclarationKey>), E>
    where
        R: NativeBoundaryResolver<E>,
    {
        match self {
            Self::Concrete(id) => resolver
                .resolve_key(id)
                .map(|key| (DecodedOwnerKind::Concrete, key)),
            Self::GenericTemplate(id) => resolver
                .resolve_key(id)
                .map(|key| (DecodedOwnerKind::GenericTemplate, key)),
        }
    }
}

impl WireEncode for DecodedNativeBoundaryNominalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_value_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedNativeBoundaryNominalOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Concrete)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::GenericTemplate)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedNativeBoundaryFieldDefinition {
    field: DecodedPersistentId<PersistentFieldId>,
    ty: DecodedSignatureTypeKey,
}

impl DecodedNativeBoundaryFieldDefinition {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryFieldDefinition, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        let key = resolver
            .resolve_key(self.field)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        let ty = self
            .ty
            .resolve(resolver)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        NativeBoundaryFieldDefinition::new(&key, ty)
            .map_err(NativeBoundaryResolutionError::Definition)
    }
}

impl WireEncode for DecodedNativeBoundaryFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

impl WireDecode for DecodedNativeBoundaryFieldDefinition {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            ty: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedNativeBoundaryVariantFieldDefinition {
    field: DecodedPersistentId<PersistentEnumVariantFieldId>,
    ty: DecodedSignatureTypeKey,
}

impl DecodedNativeBoundaryVariantFieldDefinition {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryVariantFieldDefinition, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        let key = resolver
            .resolve_key(self.field)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        let ty = self
            .ty
            .resolve(resolver)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        NativeBoundaryVariantFieldDefinition::new(&key, ty)
            .map_err(NativeBoundaryResolutionError::Definition)
    }
}

impl WireEncode for DecodedNativeBoundaryVariantFieldDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.ty.encode(encoder)
    }
}

impl WireDecode for DecodedNativeBoundaryVariantFieldDefinition {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            ty: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedNativeBoundaryVariantDefinition {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    fields: Vec<DecodedNativeBoundaryVariantFieldDefinition>,
}

impl DecodedNativeBoundaryVariantDefinition {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryVariantDefinition, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        let key = resolver
            .resolve_key(self.variant)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        let fields = resolve_sequence(self.fields, |field| field.resolve(resolver))?;
        NativeBoundaryVariantDefinition::new(&key, fields)
            .map_err(NativeBoundaryResolutionError::Definition)
    }
}

impl WireEncode for DecodedNativeBoundaryVariantDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.fields)
    }
}

impl WireDecode for DecodedNativeBoundaryVariantDefinition {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            fields: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedNativeBoundaryVariantFieldDefinition::decode(decoder)
                })
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNativeBoundaryCLayoutPolicy {
    NotCLayout,
    CLayout {
        aligned: DecodedCLayoutOverride,
        packed: DecodedCLayoutOverride,
    },
}

impl From<DecodedNativeBoundaryCLayoutPolicy> for NativeBoundaryCLayoutPolicy {
    fn from(value: DecodedNativeBoundaryCLayoutPolicy) -> Self {
        match value {
            DecodedNativeBoundaryCLayoutPolicy::NotCLayout => Self::NotCLayout,
            DecodedNativeBoundaryCLayoutPolicy::CLayout { aligned, packed } => Self::CLayout {
                aligned: aligned.into(),
                packed: packed.into(),
            },
        }
    }
}

impl WireEncode for DecodedNativeBoundaryCLayoutPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCLayout => encode_empty_sum(encoder, 1),
            Self::CLayout { aligned, packed } => encode_two_value_sum(encoder, 2, aligned, packed),
        }
    }
}

impl WireDecode for DecodedNativeBoundaryCLayoutPolicy {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::NotCLayout)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::CLayout {
                    aligned: decoder.field(1, DecodedCLayoutOverride::decode)?,
                    packed: decoder.field(2, DecodedCLayoutOverride::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNativeBoundaryNominalShape {
    Reference,
    Struct {
        c_layout: DecodedNativeBoundaryCLayoutPolicy,
        fields: Vec<DecodedNativeBoundaryFieldDefinition>,
    },
    Enum {
        variants: Vec<DecodedNativeBoundaryVariantDefinition>,
    },
}

impl DecodedNativeBoundaryNominalShape {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryNominalShape, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        match self {
            Self::Reference => Ok(NativeBoundaryNominalShape::Reference),
            Self::Struct { c_layout, fields } => Ok(NativeBoundaryNominalShape::Struct {
                c_layout: c_layout.into(),
                fields: resolve_sequence(fields, |field| field.resolve(resolver))?,
            }),
            Self::Enum { variants } => Ok(NativeBoundaryNominalShape::Enum {
                variants: resolve_sequence(variants, |variant| variant.resolve(resolver))?,
            }),
        }
    }
}

impl WireEncode for DecodedNativeBoundaryNominalShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Reference => encode_empty_sum(encoder, 1),
            Self::Struct { c_layout, fields } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                c_layout.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, fields)
            }
            Self::Enum { variants } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
        }
    }
}

impl WireDecode for DecodedNativeBoundaryNominalShape {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Reference)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Struct {
                    c_layout: decoder.field(1, DecodedNativeBoundaryCLayoutPolicy::decode)?,
                    fields: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedNativeBoundaryFieldDefinition::decode(decoder)
                        })
                    })?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::Enum {
                    variants: decoder.field(1, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedNativeBoundaryVariantDefinition::decode(decoder)
                        })
                    })?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedNativeBoundaryTypeDefinitionRecord {
    owner: DecodedNativeBoundaryNominalOwner,
    type_parameter_count: u32,
    shape: DecodedNativeBoundaryNominalShape,
}

impl DecodedNativeBoundaryTypeDefinitionRecord {
    pub const fn owner(&self) -> DecodedNativeBoundaryNominalOwner {
        self.owner
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryTypeDefinitionRecord, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        let (decoded_owner_kind, declaration) = self
            .owner
            .resolve(resolver)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        let shape = self.shape.resolve(resolver)?;
        let type_parameter_count = resolver
            .native_boundary_type_parameter_count(&declaration)
            .map_err(NativeBoundaryResolutionError::Reference)?;
        let record =
            NativeBoundaryTypeDefinitionRecord::new(&declaration, &[type_parameter_count], shape)
                .map_err(NativeBoundaryResolutionError::Definition)?;
        let actual_owner_kind = match record.owner() {
            NativeBoundaryNominalOwner::Concrete(_) => DecodedOwnerKind::Concrete,
            NativeBoundaryNominalOwner::GenericTemplate(_) => DecodedOwnerKind::GenericTemplate,
        };
        if decoded_owner_kind != actual_owner_kind {
            return Err(NativeBoundaryResolutionError::OwnerKindMismatch);
        }
        if self.type_parameter_count != record.type_parameter_count() {
            return Err(NativeBoundaryResolutionError::TypeParameterCountMismatch {
                expected: record.type_parameter_count(),
                actual: self.type_parameter_count,
            });
        }
        Ok(record)
    }

    pub fn resolve_with_external_core<R, E>(
        self,
        resolver: &mut R,
        external_source_types: &[PersistentTypeId],
        external_generic_types: &[PersistentGenericTypeId],
    ) -> Result<NativeBoundaryTypeDefinitionRecord, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        match self.owner {
            DecodedNativeBoundaryNominalOwner::Concrete(id)
                if external_source_types
                    .binary_search_by(|candidate| candidate.as_array().cmp(id.as_array()))
                    .is_ok() =>
            {
                let owner = resolver
                    .resolve(id)
                    .map_err(NativeBoundaryResolutionError::Reference)?;
                let shape = self.shape.resolve(resolver)?;
                NativeBoundaryTypeDefinitionRecord::from_external_core_source(
                    owner,
                    self.type_parameter_count,
                    shape,
                )
                .map_err(NativeBoundaryResolutionError::ExternalCoreDefinition)
            }
            DecodedNativeBoundaryNominalOwner::GenericTemplate(id)
                if external_generic_types
                    .binary_search_by(|candidate| candidate.as_array().cmp(id.as_array()))
                    .is_ok() =>
            {
                resolver
                    .resolve(id)
                    .map_err(NativeBoundaryResolutionError::Reference)?;
                Err(NativeBoundaryResolutionError::ExternalCoreGenericUnavailable)
            }
            DecodedNativeBoundaryNominalOwner::Concrete(_)
            | DecodedNativeBoundaryNominalOwner::GenericTemplate(_) => self.resolve(resolver),
        }
    }
}

impl WireEncode for DecodedNativeBoundaryTypeDefinitionRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.type_parameter_count))?;
        encoder.field(3)?;
        self.shape.encode(encoder)
    }
}

impl WireDecode for DecodedNativeBoundaryTypeDefinitionRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedNativeBoundaryNominalOwner::decode)?,
            type_parameter_count: decoder.field(2, Decoder::u32)?,
            shape: decoder.field(3, DecodedNativeBoundaryNominalShape::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeBoundaryResolutionError<E> {
    Reference(E),
    Definition(NativeBoundaryDefinitionError),
    ExternalCoreDefinition(ExternalCoreNativeBoundaryDefinitionError),
    ExternalCoreGenericUnavailable,
    OwnerKindMismatch,
    TypeParameterCountMismatch { expected: u32, actual: u32 },
    Allocation,
}

impl<E: fmt::Display> fmt::Display for NativeBoundaryResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
            Self::ExternalCoreDefinition(error) => error.fmt(formatter),
            Self::ExternalCoreGenericUnavailable => formatter
                .write_str("external core generic native-boundary types are unavailable in M23-3"),
            Self::OwnerKindMismatch => {
                formatter.write_str("native boundary owner kind does not match its declaration")
            }
            Self::TypeParameterCountMismatch { expected, actual } => write!(
                formatter,
                "native boundary type parameter count mismatch: expected {expected}, found {actual}"
            ),
            Self::Allocation => formatter.write_str("failed to allocate native boundary records"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NativeBoundaryResolutionError<E> {}

fn resolve_sequence<T, U, E>(
    values: Vec<T>,
    mut resolve: impl FnMut(T) -> Result<U, NativeBoundaryResolutionError<E>>,
) -> Result<Vec<U>, NativeBoundaryResolutionError<E>> {
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(values.len())
        .map_err(|_| NativeBoundaryResolutionError::Allocation)?;
    for value in values {
        resolved.push(resolve(value)?);
    }
    Ok(resolved)
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
