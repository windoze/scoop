use super::*;
use scoop_identity::{DecodedOptionalExactOwner, Effect, PersistentIdResolver};

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

/// Mirrors the unchanged exact-signature wire while retaining parameter
/// count for pre-allocation validation budgeting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMirBridgeCallableSignatureV1 {
    effect: Effect,
    receiver: DecodedOptionalExactOwner,
    parameters: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    result: DecodedPersistentId<PersistentExactTypeId>,
    gc_effect: crate::GcEffect,
}
impl DecodedMirBridgeCallableSignatureV1 {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<MirBridgeCallableSignatureV1, MirCallableBridgeError> {
        meter
            .charge_work(4, &WirePath::root())
            .map_err(MirCallableBridgeError::Resource)?;
        let receiver = self.receiver.resolve(graph)?.into_option();
        let parameters =
            super::super::wire::resolve_sequence(self.parameters, graph, meter, |id, graph, _| {
                Ok(graph.resolve(id)?)
            })
            .map_err(MirCallableBridgeError::Type)?;
        let result = graph.resolve(self.result)?;
        Ok(MirBridgeCallableSignatureV1::new(
            ExactCallableSignature::new(self.effect, receiver, parameters, result),
            self.gc_effect,
        ))
    }
}
impl WireDecode for DecodedMirBridgeCallableSignatureV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let (effect, receiver, parameters, result) = decoder.field(1, |decoder| {
            decoder.expect_map(4)?;
            let effect = decoder.field(1, |decoder| match decoder.unsigned()? {
                1 => Ok(Effect::Ordinary),
                2 => Ok(Effect::Suspend),
                tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
            })?;
            let receiver = decoder.field(2, DecodedOptionalExactOwner::decode)?;
            let parameters = decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?;
            let result = decoder.field(4, DecodedPersistentId::decode)?;
            Ok((effect, receiver, parameters, result))
        })?;
        let gc_effect = decoder.field(2, |decoder| {
            decoder.expect_map(1)?;
            match decoder.field(0, Decoder::unsigned)? {
                1 => Ok(crate::GcEffect::Managed),
                2 => Ok(crate::GcEffect::NoGc),
                tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
            }
        })?;
        Ok(Self {
            effect,
            receiver,
            parameters,
            result,
            gc_effect,
        })
    }
}
impl WireEncode for DecodedMirBridgeCallableSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.map(4)?;
        encoder.field(1)?;
        self.effect.encode(encoder)?;
        encoder.field(2)?;
        self.receiver.encode(encoder)?;
        encoder.field(3)?;
        sequence(encoder, &self.parameters)?;
        encoder.field(4)?;
        self.result.encode(encoder)?;
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
