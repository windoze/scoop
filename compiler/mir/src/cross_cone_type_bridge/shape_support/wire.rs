use super::*;
use scoop_identity::PersistentIdResolver;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedBoxed {
    Available(DecodedPersistentId<PersistentExactTypeId>),
    ReferenceNominalRequiresNoBox,
}
macro_rules! encode_boxed {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::Available(exact) => {
                        tag(encoder, 2, 1)?;
                        encoder.field(1)?;
                        exact.encode(encoder)
                    }
                    Self::ReferenceNominalRequiresNoBox => tag(encoder, 1, 2),
                }
            }
        }
    };
}
encode_boxed!(MirBoxedShapeSupportV1);
encode_boxed!(DecodedBoxed);
impl WireDecode for DecodedBoxed {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                fields(decoder, count, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Available)
            }
            2 => {
                fields(decoder, count, 1)?;
                Ok(Self::ReferenceNominalRequiresNoBox)
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedParamFreeMirShapeSupportV1 {
    source: DecodedPersistentId<PersistentTypeId>,
    exact: DecodedPersistentId<PersistentExactTypeId>,
    boxed: DecodedBoxed,
    coroutine_step: DecodedPersistentId<PersistentExactTypeId>,
    coroutine_slot: DecodedPersistentId<PersistentExactTypeId>,
}
impl DecodedParamFreeMirShapeSupportV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        types: &CanonicalParamFreeMirTypeExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<ParamFreeMirShapeSupportV1, MirShapeSupportError> {
        meter.charge_work(5, &WirePath::root())?;
        let source = identities.resolve(self.source)?;
        let exact = identities.resolve(self.exact)?;
        let boxed = match self.boxed {
            DecodedBoxed::Available(exact) => {
                MirBoxedShapeSupportV1::Available(identities.resolve(exact)?)
            }
            DecodedBoxed::ReferenceNominalRequiresNoBox => {
                MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox
            }
        };
        let coroutine_step = identities.resolve(self.coroutine_step)?;
        let coroutine_slot = identities.resolve(self.coroutine_slot)?;
        ParamFreeMirShapeSupportV1::try_new(
            MirShapeSupportAuthority { identities, types },
            source,
            exact,
            boxed,
            coroutine_step,
            coroutine_slot,
            meter,
        )
    }
}
macro_rules! encode_record {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(5)?;
                encoder.field(1)?;
                self.source.encode(encoder)?;
                encoder.field(2)?;
                self.exact.encode(encoder)?;
                encoder.field(3)?;
                self.boxed.encode(encoder)?;
                encoder.field(4)?;
                self.coroutine_step.encode(encoder)?;
                encoder.field(5)?;
                self.coroutine_slot.encode(encoder)
            }
        }
    };
}
encode_record!(ParamFreeMirShapeSupportV1);
encode_record!(DecodedParamFreeMirShapeSupportV1);
impl WireDecode for DecodedParamFreeMirShapeSupportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            source: decoder.field(1, DecodedPersistentId::decode)?,
            exact: decoder.field(2, DecodedPersistentId::decode)?,
            boxed: decoder.field(3, DecodedBoxed::decode)?,
            coroutine_step: decoder.field(4, DecodedPersistentId::decode)?,
            coroutine_slot: decoder.field(5, DecodedPersistentId::decode)?,
        })
    }
}
