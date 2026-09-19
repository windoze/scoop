use super::*;
use crate::{
    DecodedNominalSupportCallableInterfaceV1, DecodedNominalSupportConstructorInterfaceV1,
    DecodedNominalSupportPropertyInterfaceV1,
};
use scoop_wire::WireErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNestedSourceSupportV1 {
    Callable(Box<DecodedNominalSupportCallableInterfaceV1>),
    Constructor(Box<DecodedNominalSupportConstructorInterfaceV1>),
    Property(Box<DecodedNominalSupportPropertyInterfaceV1>),
    NestedNominal(Box<DecodedNominalSupportNestedInterfaceV1>),
}
impl DecodedNestedSourceSupportV1 {
    pub(super) fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<NestedSourceSupportV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;
        meter
            .check_semantic_depth(depth, &WirePath::root())
            .map_err(Error::Resource)?;
        Ok(match self {
            Self::Callable(record) => NestedSourceSupportV1::Callable(Box::new(
                record.resolve(resolver, meter).map_err(Error::Callable)?,
            )),
            Self::Constructor(record) => NestedSourceSupportV1::Constructor(Box::new(
                record.resolve(resolver, meter).map_err(Error::Callable)?,
            )),
            Self::Property(record) => NestedSourceSupportV1::Property(Box::new(
                record.resolve(resolver, meter).map_err(Error::Property)?,
            )),
            Self::NestedNominal(record) => NestedSourceSupportV1::NestedNominal(Box::new(
                record.resolve_at(resolver, meter, depth + 1)?,
            )),
        })
    }
}
impl WireDecode for DecodedNestedSourceSupportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => DecodedNominalSupportCallableInterfaceV1::decode_fields(decoder)
                .map(|record| Self::Callable(Box::new(record))),
            2 => DecodedNominalSupportConstructorInterfaceV1::decode_fields(decoder)
                .map(|record| Self::Constructor(Box::new(record))),
            3 => DecodedNominalSupportPropertyInterfaceV1::decode_fields(decoder)
                .map(|record| Self::Property(Box::new(record))),
            4 => DecodedNominalSupportNestedInterfaceV1::decode_fields(decoder)
                .map(|record| Self::NestedNominal(Box::new(record))),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedNestedSourceSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(record) => {
                wire::tag(encoder, 4, 1)?;
                record.encode_fields(encoder)
            }
            Self::Constructor(record) => {
                wire::tag(encoder, 4, 2)?;
                record.encode_fields(encoder)
            }
            Self::Property(record) => {
                wire::tag(encoder, 4, 3)?;
                record.encode_fields(encoder)
            }
            Self::NestedNominal(record) => {
                wire::tag(encoder, 4, 4)?;
                record.encode_fields(encoder)
            }
        }
    }
}
