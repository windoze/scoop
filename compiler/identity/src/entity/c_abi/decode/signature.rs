use scoop_wire::{
    Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, domain_separated_cbor_hash,
};

use super::{CanonicalCAbiResolutionError, DecodedCanonicalCStorageType};
use crate::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayoutFingerprint, CanonicalCAbiParameter,
    CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprint,
    CanonicalCAbiSignatureFingerprintRecord, DecodedPersistentId, PersistentExactTypeId,
    PersistentIdResolver, PersistentKeyResolver, TargetCallingConvention,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedCanonicalCAbiParameter {
    source_exact_type: DecodedPersistentId<PersistentExactTypeId>,
    storage: DecodedCanonicalCStorageType,
}

impl DecodedCanonicalCAbiParameter {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiParameter, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let source_exact_type = resolver
            .resolve(self.source_exact_type)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        let storage = self
            .storage
            .resolve(resolver)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        CanonicalCAbiParameter::new(source_exact_type, storage)
            .map_err(CanonicalCAbiResolutionError::Shape)
    }
}

impl WireEncode for DecodedCanonicalCAbiParameter {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source_exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.storage.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalCAbiParameter {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            source_exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            storage: decoder.field(2, DecodedCanonicalCStorageType::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCanonicalCAbiReturn {
    Void,
    Value {
        source_exact_type: DecodedPersistentId<PersistentExactTypeId>,
        storage: DecodedCanonicalCStorageType,
    },
}

impl DecodedCanonicalCAbiReturn {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiReturn, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        match self {
            Self::Void => Ok(CanonicalCAbiReturn::Void),
            Self::Value {
                source_exact_type,
                storage,
            } => {
                let source_exact_type = resolver
                    .resolve(source_exact_type)
                    .map_err(CanonicalCAbiResolutionError::Reference)?;
                let storage = storage
                    .resolve(resolver)
                    .map_err(CanonicalCAbiResolutionError::Reference)?;
                CanonicalCAbiReturn::value(source_exact_type, storage)
                    .map_err(CanonicalCAbiResolutionError::Shape)
            }
        }
    }
}

impl WireEncode for DecodedCanonicalCAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Void => super::encode_empty_sum(encoder, 1),
            Self::Value {
                source_exact_type,
                storage,
            } => super::encode_two_value_sum(encoder, 2, source_exact_type, storage),
        }
    }
}

impl WireDecode for DecodedCanonicalCAbiReturn {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = super::decode_sum_header(decoder)?;
        match tag {
            1 => {
                super::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Void)
            }
            2 => {
                super::expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Value {
                    source_exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                    storage: decoder.field(2, DecodedCanonicalCStorageType::decode)?,
                })
            }
            tag => Err(super::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCAbiFunctionSignature {
    calling_convention: TargetCallingConvention,
    parameters: Vec<DecodedCanonicalCAbiParameter>,
    result: DecodedCanonicalCAbiReturn,
}

impl DecodedCanonicalCAbiFunctionSignature {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiFunctionSignature, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let parameters = self
            .parameters
            .into_iter()
            .map(|parameter| parameter.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.result.resolve(resolver)?;
        match self.calling_convention {
            TargetCallingConvention::Cdecl => {
                Ok(CanonicalCAbiFunctionSignature::cdecl(parameters, result))
            }
        }
    }
}

impl WireEncode for DecodedCanonicalCAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalCAbiFunctionSignature {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            calling_convention: decoder.field(1, TargetCallingConvention::decode)?,
            parameters: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCanonicalCAbiParameter::decode(decoder))
            })?,
            result: decoder.field(3, DecodedCanonicalCAbiReturn::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCAbiSignatureFingerprintRecord {
    fingerprint: DecodedPersistentId<CanonicalCAbiSignatureFingerprint>,
    signature: DecodedCanonicalCAbiFunctionSignature,
}

impl DecodedCanonicalCAbiSignatureFingerprintRecord {
    pub const fn decoded_fingerprint(
        &self,
    ) -> DecodedPersistentId<CanonicalCAbiSignatureFingerprint> {
        self.fingerprint
    }

    /// Recomputes the typed fingerprint directly from the canonical decoded
    /// preimage. Persistent references remain untrusted until `resolve`.
    pub fn candidate_fingerprint(&self) -> Result<CanonicalCAbiSignatureFingerprint, HashError> {
        domain_separated_cbor_hash("scoop-c-abi-signature-v1", &self.signature)
            .map(|digest| CanonicalCAbiSignatureFingerprint(*digest.as_array()))
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiSignatureFingerprintRecord, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let signature = self.signature.resolve(resolver)?;
        let record = CanonicalCAbiSignatureFingerprintRecord::new(signature)
            .map_err(CanonicalCAbiResolutionError::Hash)?;
        self.fingerprint
            .verify(record.fingerprint)
            .map_err(CanonicalCAbiResolutionError::SignatureFingerprint)?;
        Ok(record)
    }

    pub fn resolve_verified<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiSignatureFingerprintRecord, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentKeyResolver<
                CanonicalCAbiSignatureFingerprint,
                CanonicalCAbiFunctionSignature,
                Error = E,
            >,
    {
        let fingerprint = resolver
            .resolve(self.fingerprint)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        let signature = resolver
            .resolve_key(self.fingerprint)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        Ok(CanonicalCAbiSignatureFingerprintRecord::from_verified(
            fingerprint,
            signature,
        ))
    }
}

impl WireEncode for DecodedCanonicalCAbiSignatureFingerprintRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalCAbiSignatureFingerprintRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            fingerprint: decoder.field(1, DecodedPersistentId::decode)?,
            signature: decoder.field(2, DecodedCanonicalCAbiFunctionSignature::decode)?,
        })
    }
}

#[cfg(test)]
mod tests;
