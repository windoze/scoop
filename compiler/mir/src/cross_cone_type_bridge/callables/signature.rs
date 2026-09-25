use super::*;
use scoop_identity::DecodedExactCallableSignature;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBridgeCallableSignatureV1 {
    exact: ExactCallableSignature,
    gc_effect: crate::GcEffect,
}
impl MirBridgeCallableSignatureV1 {
    pub fn new(exact: ExactCallableSignature, gc_effect: crate::GcEffect) -> Self {
        Self { exact, gc_effect }
    }
    pub const fn exact(&self) -> &ExactCallableSignature {
        &self.exact
    }
    pub const fn gc_effect(&self) -> crate::GcEffect {
        self.gc_effect
    }
}
impl WireEncode for MirBridgeCallableSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        tag(
            encoder,
            1,
            match self.gc_effect {
                crate::GcEffect::Managed => 1,
                crate::GcEffect::NoGc => 2,
            },
        )
    }
}

/// The shared exact-signature constituent retains its original wire and
/// resolves each parameter through its typed exact-type reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMirBridgeCallableSignatureV1 {
    exact: DecodedExactCallableSignature,
    gc_effect: crate::GcEffect,
}
impl DecodedMirBridgeCallableSignatureV1 {
    pub(in crate::cross_cone_type_bridge) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirBridgeCallableSignatureV1, MirCallableBridgeError> {
        let exact = self
            .exact
            .resolve(graph)
            .map_err(MirCallableBridgeError::ExactSignature)?;
        Ok(MirBridgeCallableSignatureV1::new(exact, self.gc_effect))
    }
}
impl WireDecode for DecodedMirBridgeCallableSignatureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let exact = decoder.field(1, DecodedExactCallableSignature::decode)?;
        let gc_effect = decoder.field(2, |decoder| {
            decoder.expect_map(1)?;
            match decoder.field(0, Decoder::unsigned)? {
                1 => Ok(crate::GcEffect::Managed),
                2 => Ok(crate::GcEffect::NoGc),
                tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
            }
        })?;
        Ok(Self { exact, gc_effect })
    }
}
impl WireEncode for DecodedMirBridgeCallableSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        tag(
            encoder,
            1,
            match self.gc_effect {
                crate::GcEffect::Managed => 1,
                crate::GcEffect::NoGc => 2,
            },
        )
    }
}
