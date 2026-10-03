use scoop_identity::{
    ConeIdentity, DecodedOptionalSignatureType, DecodedPersistentId, EnumVariantFieldKey,
    EnumVariantIdentityKey, FieldIdentityKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeId, SourceContextKey, SourceDeclarationKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;
use crate::{
    DecodedClassRepresentationFieldV1, DecodedDeclarationAccessSourceV1,
    DecodedEnumRepresentationFieldV1, DecodedStructRepresentationFieldV1,
    SignatureTypeReferenceResolver,
};

pub trait NominalRepresentationResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
    + PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentFieldId, FieldIdentityKey, Error = E>
    + PersistentKeyResolver<PersistentEnumVariantId, EnumVariantIdentityKey, Error = E>
    + PersistentKeyResolver<PersistentEnumVariantFieldId, EnumVariantFieldKey, Error = E>
{
}
impl<R, E> NominalRepresentationResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
        + PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentFieldId, FieldIdentityKey, Error = E>
        + PersistentKeyResolver<PersistentEnumVariantId, EnumVariantIdentityKey, Error = E>
        + PersistentKeyResolver<PersistentEnumVariantFieldId, EnumVariantFieldKey, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumRepresentationVariantV1 {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    fields: Vec<DecodedEnumRepresentationFieldV1>,
    gc: ExactTypeGcV1,
}

impl DecodedEnumRepresentationVariantV1 {
    pub fn resolve<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumRepresentationVariantV1, NominalRepresentationResolutionError<E>> {
        use NominalRepresentationResolutionError as Error;
        let key = resolver
            .resolve_key(self.variant)
            .map_err(Error::Reference)?;
        let mut fields = reserve(self.fields.len())?;
        for field in self.fields {
            fields.push(field.resolve(resolver).map_err(Error::EnumField)?);
        }
        let record =
            EnumRepresentationVariantV1::try_new(&key, fields, self.gc).map_err(Error::Record)?;
        self.variant
            .verify(record.variant())
            .map_err(Error::VariantIdentity)?;
        Ok(record)
    }
}

impl WireEncode for DecodedEnumRepresentationVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.fields)?;
        encoder.field(3)?;
        self.gc.encode(encoder)
    }
}
impl WireDecode for DecodedEnumRepresentationVariantV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            fields: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedEnumRepresentationFieldV1::decode(decoder))
            })?,
            gc: decoder.field(3, ExactTypeGcV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNominalRepresentationShapeV1 {
    Struct {
        fields: Vec<DecodedStructRepresentationFieldV1>,
        c_layout_policy: NominalCLayoutPolicyV1,
    },
    Enum {
        variants: Vec<DecodedEnumRepresentationVariantV1>,
    },
    Class {
        base: DecodedOptionalSignatureType,
        declared_fields: Vec<DecodedClassRepresentationFieldV1>,
    },
    Interface,
    Object {
        backing_class: DecodedPersistentId<PersistentTypeId>,
        declared_fields: Vec<DecodedClassRepresentationFieldV1>,
    },
    Intrinsic {
        representation: NominalIntrinsicRepresentationV1,
    },
}

impl DecodedNominalRepresentationShapeV1 {
    pub fn resolve<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalRepresentationShapeV1, NominalRepresentationResolutionError<E>> {
        use NominalRepresentationResolutionError as Error;
        Ok(match self {
            Self::Struct {
                fields,
                c_layout_policy,
            } => {
                let mut values = reserve(fields.len())?;
                for field in fields {
                    values.push(field.resolve(resolver).map_err(Error::Field)?);
                }
                NominalRepresentationShapeV1::Struct {
                    fields: values,
                    c_layout_policy,
                }
            }
            Self::Enum { variants } => {
                let mut values = reserve(variants.len())?;
                for variant in variants {
                    values.push(variant.resolve(resolver)?);
                }
                NominalRepresentationShapeV1::Enum { variants: values }
            }
            Self::Class {
                base,
                declared_fields,
            } => NominalRepresentationShapeV1::Class {
                base: base.resolve(resolver).map_err(Error::Reference)?,
                declared_fields: resolve_class_fields(declared_fields, resolver)?,
            },
            Self::Interface => NominalRepresentationShapeV1::Interface,
            Self::Object {
                backing_class,
                declared_fields,
            } => NominalRepresentationShapeV1::Object {
                backing_class: resolver.resolve(backing_class).map_err(Error::Reference)?,
                declared_fields: resolve_class_fields(declared_fields, resolver)?,
            },
            Self::Intrinsic { representation } => {
                NominalRepresentationShapeV1::Intrinsic { representation }
            }
        })
    }
}

fn resolve_class_fields<R: NominalRepresentationResolver<E>, E>(
    fields: Vec<DecodedClassRepresentationFieldV1>,
    resolver: &mut R,
) -> Result<Vec<ClassRepresentationFieldV1>, NominalRepresentationResolutionError<E>> {
    let mut values = reserve(fields.len())?;
    for field in fields {
        values.push(
            field
                .resolve(resolver)
                .map_err(NominalRepresentationResolutionError::Field)?,
        );
    }
    Ok(values)
}

fn reserve<T, E>(count: usize) -> Result<Vec<T>, NominalRepresentationResolutionError<E>> {
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &scoop_wire::WirePath::root())
        .map_err(NominalRepresentationResolutionError::Allocation)?;
    Ok(values)
}

impl WireEncode for DecodedNominalRepresentationShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Struct {
                fields,
                c_layout_policy,
            } => {
                wire::tag(encoder, 3, 1)?;
                encoder.field(1)?;
                wire::sequence(encoder, fields)?;
                encoder.field(2)?;
                c_layout_policy.encode(encoder)
            }
            Self::Enum { variants } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                wire::sequence(encoder, variants)
            }
            Self::Class {
                base,
                declared_fields,
            } => {
                wire::tag(encoder, 3, 3)?;
                encoder.field(1)?;
                base.encode(encoder)?;
                encoder.field(2)?;
                wire::sequence(encoder, declared_fields)
            }
            Self::Interface => wire::tag(encoder, 1, 4),
            Self::Object {
                backing_class,
                declared_fields,
            } => {
                wire::tag(encoder, 3, 5)?;
                encoder.field(1)?;
                backing_class.encode(encoder)?;
                encoder.field(2)?;
                wire::sequence(encoder, declared_fields)
            }
            Self::Intrinsic { representation } => {
                wire::tag(encoder, 2, 6)?;
                encoder.field(1)?;
                representation.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedNominalRepresentationShapeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 3)?;
                Ok(Self::Struct {
                    fields: decoder.field(1, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedStructRepresentationFieldV1::decode(decoder)
                        })
                    })?,
                    c_layout_policy: decoder.field(2, NominalCLayoutPolicyV1::decode)?,
                })
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::Enum {
                    variants: decoder.field(1, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedEnumRepresentationVariantV1::decode(decoder)
                        })
                    })?,
                })
            }
            3 => {
                wire::expect_fields(decoder, fields, 3)?;
                Ok(Self::Class {
                    base: decoder.field(1, DecodedOptionalSignatureType::decode)?,
                    declared_fields: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedClassRepresentationFieldV1::decode(decoder)
                        })
                    })?,
                })
            }
            4 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Interface)
            }
            5 => {
                wire::expect_fields(decoder, fields, 3)?;
                Ok(Self::Object {
                    backing_class: decoder.field(1, DecodedPersistentId::decode)?,
                    declared_fields: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedClassRepresentationFieldV1::decode(decoder)
                        })
                    })?,
                })
            }
            6 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::Intrinsic {
                    representation: decoder.field(1, NominalIntrinsicRepresentationV1::decode)?,
                })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalRepresentationSupportV1 {
    owner: DecodedPersistentId<PersistentTypeId>,
    declaration_access: DecodedDeclarationAccessSourceV1,
    shape: DecodedNominalRepresentationShapeV1,
}

impl DecodedNominalRepresentationSupportV1 {
    pub fn resolve<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalRepresentationSupportV1, NominalRepresentationResolutionError<E>> {
        let key = resolver
            .resolve_key(self.owner)
            .map_err(NominalRepresentationResolutionError::Reference)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(NominalRepresentationResolutionError::Access)?;
        let shape = self.shape.resolve(resolver)?;
        let record = NominalRepresentationSupportV1::try_new(&key, access, shape)
            .map_err(NominalRepresentationResolutionError::Record)?;
        self.owner
            .verify(record.owner())
            .map_err(NominalRepresentationResolutionError::OwnerIdentity)?;
        Ok(record)
    }
}

impl WireEncode for DecodedNominalRepresentationSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.shape.encode(encoder)
    }
}
impl WireDecode for DecodedNominalRepresentationSupportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            shape: decoder.field(3, DecodedNominalRepresentationShapeV1::decode)?,
        })
    }
}
