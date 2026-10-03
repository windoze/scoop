use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{ExactCallableSignature, OptionalExactOwner};
use crate::{DecodedPersistentId, Effect, PersistentExactTypeId, PersistentIdResolver};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedOptionalExactOwner {
    Absent,
    Present(DecodedPersistentId<PersistentExactTypeId>),
}

impl DecodedOptionalExactOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<OptionalExactOwner, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Absent => Ok(OptionalExactOwner::Absent),
            Self::Present(owner) => resolver.resolve(owner).map(OptionalExactOwner::Present),
        }
    }
}

impl WireEncode for DecodedOptionalExactOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty_sum(encoder, 1),
            Self::Present(owner) => encode_value_sum(encoder, 2, owner),
        }
    }
}

impl WireDecode for DecodedOptionalExactOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedExactCallableSignature {
    effect: Effect,
    receiver: DecodedOptionalExactOwner,
    parameters: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    result: DecodedPersistentId<PersistentExactTypeId>,
}

impl DecodedExactCallableSignature {
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExactCallableSignature, ExactCallableSignatureResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        let receiver = match self
            .receiver
            .resolve(resolver)
            .map_err(ExactCallableSignatureResolutionError::Reference)?
        {
            OptionalExactOwner::Absent => None,
            OptionalExactOwner::Present(owner) => Some(owner),
        };
        let parameters = resolve_sequence(self.parameters, resolver)?;
        let result = resolver
            .resolve(self.result)
            .map_err(ExactCallableSignatureResolutionError::Reference)?;
        Ok(ExactCallableSignature::new(
            self.effect,
            receiver,
            parameters,
            result,
        ))
    }
}

impl WireEncode for DecodedExactCallableSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.effect.encode(encoder)?;
        encoder.field(2)?;
        self.receiver.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.parameters)?;
        encoder.field(4)?;
        self.result.encode(encoder)
    }
}

impl WireDecode for DecodedExactCallableSignature {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            effect: decoder.field(1, decode_effect)?,
            receiver: decoder.field(2, DecodedOptionalExactOwner::decode)?,
            parameters: decoder.field(3, decode_exact_ids)?,
            result: decoder.field(4, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactCallableSignatureResolutionError<E> {
    Reference(E),
    Allocation,
}

impl<E: fmt::Display> fmt::Display for ExactCallableSignatureResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Allocation => {
                formatter.write_str("failed to allocate resolved exact signature parameters")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExactCallableSignatureResolutionError<E>
{
}

fn resolve_sequence<R, E>(
    values: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    resolver: &mut R,
) -> Result<Vec<PersistentExactTypeId>, ExactCallableSignatureResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(values.len())
        .map_err(|_| ExactCallableSignatureResolutionError::Allocation)?;
    for value in values {
        resolved.push(
            resolver
                .resolve(value)
                .map_err(ExactCallableSignatureResolutionError::Reference)?,
        );
    }
    Ok(resolved)
}

fn decode_exact_ids(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedPersistentId<PersistentExactTypeId>>, WireError> {
    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
}

fn decode_effect(decoder: &mut Decoder<'_>) -> Result<Effect, WireError> {
    match decoder.unsigned()? {
        1 => Ok(Effect::Ordinary),
        2 => Ok(Effect::Suspend),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::{WireErrorKind, decode_canonical, encode};

    use super::{DecodedExactCallableSignature, ExactCallableSignatureResolutionError};
    use crate::{
        Effect, ExactCallableSignature, PersistentExactTypeId, PersistentIdMismatch,
        PersistentIdResolver,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ResolutionError;

    impl std::fmt::Display for ResolutionError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("identity is absent from the test graph")
        }
    }

    impl std::error::Error for ResolutionError {}

    struct Resolver;

    impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
        type Error = ResolutionError;

        fn resolve(
            &mut self,
            id: crate::DecodedPersistentId<PersistentExactTypeId>,
        ) -> Result<PersistentExactTypeId, Self::Error> {
            id.verify(PersistentExactTypeId([7; 32]))
                .map_err(|_: PersistentIdMismatch<PersistentExactTypeId>| ResolutionError)
        }
    }

    #[test]
    fn exact_signatures_round_trip_present_and_absent_receivers() {
        let exact = PersistentExactTypeId([7; 32]);
        for signature in [
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact),
            ExactCallableSignature::new(Effect::Suspend, Some(exact), vec![exact], exact),
        ] {
            let decoded =
                decode_canonical::<DecodedExactCallableSignature>(&encode(&signature).unwrap())
                    .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), signature);
        }
    }

    #[test]
    fn exact_signature_resolution_keeps_references_typed() {
        let signature = ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![],
            PersistentExactTypeId([8; 32]),
        );
        let decoded =
            decode_canonical::<DecodedExactCallableSignature>(&encode(&signature).unwrap())
                .unwrap();
        assert_eq!(
            decoded.resolve(&mut Resolver),
            Err(ExactCallableSignatureResolutionError::Reference(
                ResolutionError
            ))
        );
    }

    #[test]
    fn exact_signature_decoder_rejects_unknown_effect_and_owner_tags() {
        let exact = PersistentExactTypeId([7; 32]);
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact);

        let mut bad_effect = encode(&signature).unwrap();
        bad_effect[2] = 3;
        let error = decode_canonical::<DecodedExactCallableSignature>(&bad_effect).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

        let mut bad_owner = encode(&signature).unwrap();
        bad_owner[6] = 3;
        let error = decode_canonical::<DecodedExactCallableSignature>(&bad_owner).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
    }
}
