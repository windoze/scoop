mod resolution_nodes;

use std::fmt;

use scoop_identity::{DecodedPersistentId, LocalValueSelector, PersistentPropertyId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedDefaultExpressionV1, DecodedDefaultFieldRefV1, DefaultExpressionIndexError,
    DefaultExpressionReferenceResolver, DefaultExpressionResolutionError, DefaultExpressionV1,
    DefaultFieldRefResolutionError, DefaultFieldRefV1, IndexedDefaultExpressionV1,
    TemplateLocalIndexResolver, TemplateLocalSelectorResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultAssignTargetV1 {
    Local {
        local: LocalValueSelector,
    },
    Global {
        property: PersistentPropertyId,
    },
    Index {
        array: Box<DefaultExpressionV1>,
        index: Box<DefaultExpressionV1>,
    },
    Field {
        receiver: Box<DefaultExpressionV1>,
        field: DefaultFieldRefV1,
    },
}

impl DefaultAssignTargetV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultAssignTargetV1<'_>, DefaultAssignTargetIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self {
            Self::Local { local } => resolver
                .resolve_template_local_index(local)
                .map(|local_index| IndexedDefaultAssignTargetV1::Local { local_index })
                .map_err(DefaultAssignTargetIndexError::Local),
            Self::Global { property } => Ok(IndexedDefaultAssignTargetV1::Global { property }),
            Self::Index { array, index } => Ok(IndexedDefaultAssignTargetV1::Index {
                array: array.index_locals(resolver).map_err(|error| {
                    DefaultAssignTargetIndexError::Expression {
                        target_tag: 3,
                        field: 1,
                        error,
                    }
                })?,
                index: index.index_locals(resolver).map_err(|error| {
                    DefaultAssignTargetIndexError::Expression {
                        target_tag: 3,
                        field: 2,
                        error,
                    }
                })?,
            }),
            Self::Field { receiver, field } => Ok(IndexedDefaultAssignTargetV1::Field {
                receiver: receiver.index_locals(resolver).map_err(|error| {
                    DefaultAssignTargetIndexError::Expression {
                        target_tag: 4,
                        field: 1,
                        error,
                    }
                })?,
                field,
            }),
        }
    }
}

#[derive(Debug)]
pub enum IndexedDefaultAssignTargetV1<'a> {
    Local {
        local_index: u32,
    },
    Global {
        property: &'a PersistentPropertyId,
    },
    Index {
        array: IndexedDefaultExpressionV1<'a>,
        index: IndexedDefaultExpressionV1<'a>,
    },
    Field {
        receiver: IndexedDefaultExpressionV1<'a>,
        field: &'a DefaultFieldRefV1,
    },
}

impl WireEncode for IndexedDefaultAssignTargetV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local { local_index } => encode_u32_payload(encoder, 1, *local_index),
            Self::Global { property } => encode_one(encoder, 2, *property),
            Self::Index { array, index } => encode_two(encoder, 3, array, index),
            Self::Field { receiver, field } => encode_two(encoder, 4, receiver, *field),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultAssignTargetV1 {
    Local {
        local_index: u32,
    },
    Global {
        property: DecodedPersistentId<PersistentPropertyId>,
    },
    Index {
        array: Box<DecodedDefaultExpressionV1>,
        index: Box<DecodedDefaultExpressionV1>,
    },
    Field {
        receiver: Box<DecodedDefaultExpressionV1>,
        field: DecodedDefaultFieldRefV1,
    },
}

impl DecodedDefaultAssignTargetV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultAssignTargetV1, DefaultAssignTargetResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Local { local_index } => locals
                .resolve_template_local_selector(local_index)
                .map(|local| DefaultAssignTargetV1::Local { local })
                .map_err(DefaultAssignTargetResolutionError::Local),
            Self::Global { property } => resolver
                .resolve(property)
                .map(|property| DefaultAssignTargetV1::Global { property })
                .map_err(DefaultAssignTargetResolutionError::Global),
            Self::Index { array, index } => Ok(DefaultAssignTargetV1::Index {
                array: Box::new(array.resolve(resolver, locals).map_err(|error| {
                    DefaultAssignTargetResolutionError::Expression {
                        target_tag: 3,
                        field: 1,
                        error,
                    }
                })?),
                index: Box::new(index.resolve(resolver, locals).map_err(|error| {
                    DefaultAssignTargetResolutionError::Expression {
                        target_tag: 3,
                        field: 2,
                        error,
                    }
                })?),
            }),
            Self::Field { receiver, field } => Ok(DefaultAssignTargetV1::Field {
                receiver: Box::new(receiver.resolve(resolver, locals).map_err(|error| {
                    DefaultAssignTargetResolutionError::Expression {
                        target_tag: 4,
                        field: 1,
                        error,
                    }
                })?),
                field: field
                    .resolve(resolver)
                    .map_err(DefaultAssignTargetResolutionError::Field)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultAssignTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local { local_index } => encode_u32_payload(encoder, 1, *local_index),
            Self::Global { property } => encode_one(encoder, 2, property),
            Self::Index { array, index } => encode_two(encoder, 3, array.as_ref(), index.as_ref()),
            Self::Field { receiver, field } => encode_two(encoder, 4, receiver.as_ref(), field),
        }
    }
}

impl WireDecode for DecodedDefaultAssignTargetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::u32)
                    .map(|local_index| Self::Local { local_index })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|property| Self::Global { property })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Index {
                    array: decoder
                        .field(1, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                    index: decoder
                        .field(2, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Field {
                    receiver: decoder
                        .field(1, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                    field: decoder.field(2, DecodedDefaultFieldRefV1::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultAssignTargetIndexError<E> {
    Local(E),
    Expression {
        target_tag: u64,
        field: u32,
        error: DefaultExpressionIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultAssignTargetIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "cannot index assignment local: {error}"),
            Self::Expression {
                target_tag,
                field,
                error,
            } => write!(
                formatter,
                "cannot index assignment target tag {target_tag} field {field}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultAssignTargetIndexError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultAssignTargetResolutionError<E, L> {
    Local(L),
    Global(E),
    Expression {
        target_tag: u64,
        field: u32,
        error: DefaultExpressionResolutionError<E, L>,
    },
    Field(DefaultFieldRefResolutionError<E>),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultAssignTargetResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid assignment local: {error}"),
            Self::Global(error) => write!(formatter, "invalid assignment global: {error}"),
            Self::Expression {
                target_tag,
                field,
                error,
            } => write!(
                formatter,
                "invalid assignment target tag {target_tag} field {field}: {error}"
            ),
            Self::Field(error) => write!(formatter, "invalid assignment field: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultAssignTargetResolutionError<E, L>
{
}

fn encode_u32_payload(
    encoder: &mut Encoder,
    tag: u64,
    value: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(value))
}

fn encode_one(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two(
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

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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
