use scoop_identity::{DecodedPersistentId, PersistentIdResolver};
use scoop_wire::{Decoder, WireDecode, WireErrorKind};

use super::*;
use crate::ExactTypeGcV1;

mod resolve;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedExactTypeFactShapeV1 {
    Unit,
    Scalar,
    Pointer,
    Reference,
    MaybeUninit {
        value: DecodedPersistentId<PersistentExactTypeId>,
    },
    OrdinaryStruct {
        fields: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    CLayoutStruct {
        fields: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    Tuple {
        elements: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    Enum {
        variants: Vec<DecodedExactEnumVariantFactsV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExactEnumVariantFactsV1 {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    fields: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    gc: ExactTypeGcV1,
}

impl WireDecode for DecodedExactTypeFactShapeV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        let tag = d.field(0, Decoder::unsigned)?;
        Ok(match tag {
            9 => {
                wire::expect_fields(d, fields, 2)?;
                Self::MaybeUninit {
                    value: d.field(1, DecodedPersistentId::decode)?,
                }
            }
            1..=4 => {
                wire::expect_fields(d, fields, 1)?;
                match tag {
                    1 => Self::Unit,
                    2 => Self::Scalar,
                    3 => Self::Pointer,
                    4 => Self::Reference,
                    _ => unreachable!("the leaf tag was bounded above"),
                }
            }
            5..=7 => {
                wire::expect_fields(d, fields, 2)?;
                let values =
                    d.field(1, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?;
                match tag {
                    5 => Self::OrdinaryStruct { fields: values },
                    6 => Self::CLayoutStruct { fields: values },
                    7 => Self::Tuple { elements: values },
                    _ => unreachable!("the aggregate tag was bounded above"),
                }
            }
            8 => {
                wire::expect_fields(d, fields, 2)?;
                Self::Enum {
                    variants: d.field(1, |d| {
                        d.decode_array(|d, _| DecodedExactEnumVariantFactsV1::decode(d))
                    })?,
                }
            }
            tag => return Err(wire::error(d, WireErrorKind::UnknownTag { tag })),
        })
    }
}

impl WireEncode for DecodedExactTypeFactShapeV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unit => wire::tag(e, 1, 1),
            Self::Scalar => wire::tag(e, 1, 2),
            Self::Pointer => wire::tag(e, 1, 3),
            Self::Reference => wire::tag(e, 1, 4),
            Self::MaybeUninit { value } => {
                wire::tag(e, 2, 9)?;
                e.field(1)?;
                value.encode(e)
            }
            Self::OrdinaryStruct { fields } => encode::sequence_shape(e, 5, fields),
            Self::CLayoutStruct { fields } => encode::sequence_shape(e, 6, fields),
            Self::Tuple { elements } => encode::sequence_shape(e, 7, elements),
            Self::Enum { variants } => encode::sequence_shape(e, 8, variants),
        }
    }
}

impl WireDecode for DecodedExactEnumVariantFactsV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            variant: d.field(1, DecodedPersistentId::decode)?,
            fields: d.field(2, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            gc: d.field(3, ExactTypeGcV1::decode)?,
        })
    }
}

impl WireEncode for DecodedExactEnumVariantFactsV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(3)?;
        e.field(1)?;
        self.variant.encode(e)?;
        e.field(2)?;
        wire::sequence(e, &self.fields)?;
        e.field(3)?;
        self.gc.encode(e)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExactTypeFactShapeRecordV1 {
    exact: DecodedPersistentId<PersistentExactTypeId>,
    shape: DecodedExactTypeFactShapeV1,
}

impl WireDecode for DecodedExactTypeFactShapeRecordV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        Ok(Self {
            exact: d.field(1, DecodedPersistentId::decode)?,
            shape: d.field(2, DecodedExactTypeFactShapeV1::decode)?,
        })
    }
}

impl WireEncode for DecodedExactTypeFactShapeRecordV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        self.exact.encode(e)?;
        e.field(2)?;
        self.shape.encode(e)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExactTypeFactShapesV1 {
    records: Vec<DecodedExactTypeFactShapeRecordV1>,
}

impl WireDecode for DecodedCanonicalExactTypeFactShapesV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.decode_array(|d, _| DecodedExactTypeFactShapeRecordV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalExactTypeFactShapesV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(e, &self.records)
    }
}
