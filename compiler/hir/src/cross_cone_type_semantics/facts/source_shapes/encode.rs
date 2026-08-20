use super::*;

impl WireEncode for ExactTypeFactShapeV1 {
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
            Self::OrdinaryStruct { fields } => sequence_shape(e, 5, fields),
            Self::CLayoutStruct { fields } => sequence_shape(e, 6, fields),
            Self::Tuple { elements } => sequence_shape(e, 7, elements),
            Self::Enum { variants } => sequence_shape(e, 8, variants),
        }
    }
}

pub(super) fn sequence_shape(
    e: &mut Encoder,
    tag: u64,
    items: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    wire::tag(e, 2, tag)?;
    e.field(1)?;
    wire::sequence(e, items)
}

impl WireEncode for ExactEnumVariantFactsV1 {
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

impl WireEncode for ExactTypeFactShapeRecordV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        self.exact.encode(e)?;
        e.field(2)?;
        self.shape.encode(e)
    }
}

impl WireEncode for CanonicalExactTypeFactShapesV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(e, &self.records)
    }
}
