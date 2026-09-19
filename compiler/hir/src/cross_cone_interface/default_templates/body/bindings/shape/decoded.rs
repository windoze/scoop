mod resolution_nodes;

use std::{collections::BTreeSet, num::NonZeroU32};

use scoop_identity::DecodedSignatureTypeKey;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::super::values::DecodedDefaultBindingLeafV1;
use super::{
    DefaultBindingClassComponentV1, DefaultBindingShapeBuildError, DefaultBindingShapeKindV1,
    DefaultBindingShapeResolutionError, DefaultBindingShapeV1, DefaultBindingStructFieldV1,
    encode_composite, encode_empty_sum, encode_indexed_shape, encode_sequence,
    encode_single_payload, encode_tag, require_u32_len,
};
use crate::{SignatureTypeReferenceResolver, TemplateLocalSelectorResolver};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingShapeV1(DecodedDefaultBindingShapeKindV1);

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDefaultBindingShapeKindV1 {
    Binding(DecodedDefaultBindingLeafV1),
    Wildcard,
    Tuple(Vec<DecodedDefaultBindingShapeV1>),
    Struct {
        owner_type: DecodedSignatureTypeKey,
        fields: Vec<DecodedDefaultBindingStructFieldV1>,
    },
    Class {
        owner_type: DecodedSignatureTypeKey,
        components: Vec<DecodedDefaultBindingClassComponentV1>,
    },
}

impl DecodedDefaultBindingShapeV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultBindingShapeV1, DefaultBindingShapeResolutionError<E, L::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self.0 {
            DecodedDefaultBindingShapeKindV1::Binding(leaf) => leaf
                .resolve(resolver, locals)
                .map(DefaultBindingShapeV1::binding)
                .map_err(DefaultBindingShapeResolutionError::Binding),
            DecodedDefaultBindingShapeKindV1::Wildcard => Ok(DefaultBindingShapeV1::wildcard()),
            DecodedDefaultBindingShapeKindV1::Tuple(elements) => {
                require_u32_len(
                    elements.len(),
                    DefaultBindingShapeResolutionError::Shape(
                        DefaultBindingShapeBuildError::TooManyElements,
                    ),
                )?;
                let mut resolved = Vec::with_capacity(elements.len());
                for (index, element) in elements.into_iter().enumerate() {
                    resolved.push(element.resolve(resolver, locals).map_err(|error| {
                        DefaultBindingShapeResolutionError::Element {
                            index,
                            error: Box::new(error),
                        }
                    })?);
                }
                Ok(DefaultBindingShapeV1(DefaultBindingShapeKindV1::Tuple(
                    resolved,
                )))
            }
            DecodedDefaultBindingShapeKindV1::Struct { owner_type, fields } => {
                validate_struct_fields(&fields)
                    .map_err(DefaultBindingShapeResolutionError::Shape)?;
                let owner_type = owner_type
                    .resolve(resolver)
                    .map_err(DefaultBindingShapeResolutionError::StructOwnerType)?;
                let mut resolved = Vec::with_capacity(fields.len());
                for (index, field) in fields.into_iter().enumerate() {
                    resolved.push(DefaultBindingStructFieldV1 {
                        declaration_index: field.declaration_index,
                        shape: field.shape.resolve(resolver, locals).map_err(|error| {
                            DefaultBindingShapeResolutionError::StructField {
                                index,
                                error: Box::new(error),
                            }
                        })?,
                    });
                }
                Ok(DefaultBindingShapeV1(DefaultBindingShapeKindV1::Struct {
                    owner_type,
                    fields: resolved,
                }))
            }
            DecodedDefaultBindingShapeKindV1::Class {
                owner_type,
                components,
            } => {
                validate_class_components(&components)
                    .map_err(DefaultBindingShapeResolutionError::Shape)?;
                let owner_type = owner_type
                    .resolve(resolver)
                    .map_err(DefaultBindingShapeResolutionError::ClassOwnerType)?;
                let mut resolved = Vec::with_capacity(components.len());
                for (position, component) in components.into_iter().enumerate() {
                    resolved.push(DefaultBindingClassComponentV1 {
                        index: component.index,
                        shape: component.shape.resolve(resolver, locals).map_err(|error| {
                            DefaultBindingShapeResolutionError::ClassComponent {
                                index: position,
                                error: Box::new(error),
                            }
                        })?,
                    });
                }
                Ok(DefaultBindingShapeV1(DefaultBindingShapeKindV1::Class {
                    owner_type,
                    components: resolved,
                }))
            }
        }
    }
}

impl WireEncode for DecodedDefaultBindingShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            DecodedDefaultBindingShapeKindV1::Binding(leaf) => {
                encode_single_payload(encoder, 1, leaf)
            }
            DecodedDefaultBindingShapeKindV1::Wildcard => encode_empty_sum(encoder, 2),
            DecodedDefaultBindingShapeKindV1::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements)
            }
            DecodedDefaultBindingShapeKindV1::Struct { owner_type, fields } => {
                encode_composite(encoder, 4, owner_type, fields)
            }
            DecodedDefaultBindingShapeKindV1::Class {
                owner_type,
                components,
            } => encode_composite(encoder, 5, owner_type, components),
        }
    }
}

impl WireDecode for DecodedDefaultBindingShapeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultBindingLeafV1::decode)
                    .map(DecodedDefaultBindingShapeKindV1::Binding)
                    .map(Self)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self(DecodedDefaultBindingShapeKindV1::Wildcard))
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder.decode_array(|decoder, _| Self::decode(decoder))
                    })
                    .map(DecodedDefaultBindingShapeKindV1::Tuple)
                    .map(Self)
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self(DecodedDefaultBindingShapeKindV1::Struct {
                    owner_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    fields: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedDefaultBindingStructFieldV1::decode(decoder)
                        })
                    })?,
                }))
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self(DecodedDefaultBindingShapeKindV1::Class {
                    owner_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    components: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedDefaultBindingClassComponentV1::decode(decoder)
                        })
                    })?,
                }))
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingStructFieldV1 {
    declaration_index: u32,
    shape: DecodedDefaultBindingShapeV1,
}

impl WireEncode for DecodedDefaultBindingStructFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_shape(encoder, self.declaration_index, &self.shape)
    }
}

impl WireDecode for DecodedDefaultBindingStructFieldV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declaration_index: decoder.field(1, Decoder::u32)?,
            shape: decoder.field(2, DecodedDefaultBindingShapeV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingClassComponentV1 {
    index: NonZeroU32,
    shape: DecodedDefaultBindingShapeV1,
}

impl WireEncode for DecodedDefaultBindingClassComponentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_shape(encoder, self.index.get(), &self.shape)
    }
}

impl WireDecode for DecodedDefaultBindingClassComponentV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let index = decoder.field(1, Decoder::u32)?;
        Ok(Self {
            index: NonZeroU32::new(index).ok_or_else(|| integer_out_of_range(decoder))?,
            shape: decoder.field(2, DecodedDefaultBindingShapeV1::decode)?,
        })
    }
}

fn validate_struct_fields(
    fields: &[DecodedDefaultBindingStructFieldV1],
) -> Result<(), DefaultBindingShapeBuildError> {
    require_u32_len(
        fields.len(),
        DefaultBindingShapeBuildError::TooManyStructFields,
    )?;
    for (offset, pair) in fields.windows(2).enumerate() {
        if pair[0].declaration_index == pair[1].declaration_index {
            return Err(DefaultBindingShapeBuildError::DuplicateStructField {
                declaration_index: pair[0].declaration_index,
            });
        }
        if pair[0].declaration_index > pair[1].declaration_index {
            return Err(
                DefaultBindingShapeBuildError::NonCanonicalStructFieldOrder {
                    index: offset + 1,
                    previous: pair[0].declaration_index,
                    actual: pair[1].declaration_index,
                },
            );
        }
    }
    Ok(())
}

fn validate_class_components(
    components: &[DecodedDefaultBindingClassComponentV1],
) -> Result<(), DefaultBindingShapeBuildError> {
    require_u32_len(
        components.len(),
        DefaultBindingShapeBuildError::TooManyClassComponents,
    )?;
    let mut indices = BTreeSet::new();
    for component in components {
        if !indices.insert(component.index) {
            return Err(DefaultBindingShapeBuildError::DuplicateClassComponent {
                index: component.index,
            });
        }
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

fn integer_out_of_range(decoder: &Decoder<'_, '_>) -> WireError {
    wire_error(decoder, WireErrorKind::IntegerOutOfRange)
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
