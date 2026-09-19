use super::*;

/// The HIR semantic classification survives without any target byte size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirValueKindV1 {
    ZeroSizedValue,
    NonZeroValue,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirGcKindV1 {
    GcFree,
    ContainsManagedReferences,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirTypeFactsV1 {
    kind: MirValueKindV1,
    gc: MirGcKindV1,
}
impl MirTypeFactsV1 {
    pub fn try_new(kind: MirValueKindV1, gc: MirGcKindV1) -> Result<Self, MirTypeBridgeError> {
        if matches!(
            (kind, gc),
            (
                MirValueKindV1::ZeroSizedValue,
                MirGcKindV1::ContainsManagedReferences
            ) | (MirValueKindV1::Reference, MirGcKindV1::GcFree)
        ) {
            return Err(MirTypeBridgeError::ContradictoryFacts);
        }
        Ok(Self { kind, gc })
    }
    pub const fn kind(self) -> MirValueKindV1 {
        self.kind
    }
    pub const fn gc(self) -> MirGcKindV1 {
        self.gc
    }
}
impl WireEncode for MirTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.gc.encode(encoder)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedMirTypeFactsV1 {
    kind: MirValueKindV1,
    gc: MirGcKindV1,
}
impl DecodedMirTypeFactsV1 {
    pub(super) fn validate(self) -> Result<MirTypeFactsV1, MirTypeBridgeError> {
        MirTypeFactsV1::try_new(self.kind, self.gc)
    }
}
impl WireDecode for DecodedMirTypeFactsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            kind: decoder.field(1, MirValueKindV1::decode)?,
            gc: decoder.field(2, MirGcKindV1::decode)?,
        })
    }
}
impl WireEncode for DecodedMirTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.gc.encode(encoder)
    }
}
impl WireEncode for MirValueKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        tag(
            encoder,
            1,
            match self {
                Self::ZeroSizedValue => 1,
                Self::NonZeroValue => 2,
                Self::Reference => 3,
            },
        )
    }
}
impl WireDecode for MirValueKindV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::ZeroSizedValue),
            2 => Ok(Self::NonZeroValue),
            3 => Ok(Self::Reference),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for MirGcKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        tag(
            encoder,
            1,
            match self {
                Self::GcFree => 1,
                Self::ContainsManagedReferences => 2,
            },
        )
    }
}
impl WireDecode for MirGcKindV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::GcFree),
            2 => Ok(Self::ContainsManagedReferences),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
