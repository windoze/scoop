use scoop_identity::{DecodedCallableDefinitionOwner, DecodedCanonicalScoopAbiFunctionSignature};
use scoop_wire::{Decoder, WireDecode, WireErrorKind, encode_canonical_temporary};

use super::*;

#[derive(Debug)]
pub struct DecodedExactCallableAbiExportV1 {
    target: DecodedCallableDefinitionOwner,
    signature: DecodedCanonicalScoopAbiFunctionSignature,
    convention: crate::CallingConvention,
    protocol: ExactCallableProtocolV1,
    definition: crate::production::DecodedStrongShapeDefinitionV1<PersistentCallableBodyId>,
}

#[derive(Debug)]
pub struct DecodedCanonicalExactCallableAbiExportsV1 {
    records: Vec<DecodedExactCallableAbiExportV1>,
}

impl DecodedExactCallableAbiExportV1 {
    pub fn validate_against(
        self,
        expected: &ExactCallableAbiExportV1,
    ) -> Result<ExactCallableAbiExportV1, ExactCallableAbiWireError> {
        let path = WirePath::root();

        if !target_matches(self.target, expected.target()) {
            return Err(ExactCallableAbiWireError::Target);
        }
        if self.convention != expected.calling_convention()
            || self.protocol != expected.call_protocol()
        {
            return Err(ExactCallableAbiWireError::Protocol);
        }
        if !self.definition.matches_definition(expected.definition())? {
            return Err(ExactCallableAbiWireError::Definition);
        }
        // Compare the complete frozen signature product: exact receiver and
        // parameters, result, every storage field and pass mode, and GC effect.
        // No raw signature is promoted or repaired through a digest match.

        let actual = encode_canonical_temporary(&self.signature, &path)?;
        let wanted = encode_canonical_temporary(expected.canonical_signature(), &path)?;

        if actual != wanted {
            return Err(ExactCallableAbiWireError::Signature);
        }
        Ok(expected.clone())
    }
}

impl DecodedCanonicalExactCallableAbiExportsV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalExactCallableAbiExportsV1,
    ) -> Result<CanonicalExactCallableAbiExportsV1, ExactCallableAbiTableError> {
        if self.records.len() != expected.records().len() {
            return Err(ExactCallableAbiTableError::Count);
        }
        for (index, (actual, expected)) in
            self.records.into_iter().zip(expected.records()).enumerate()
        {
            actual
                .validate_against(expected)
                .map_err(|source| ExactCallableAbiTableError::Record { index, source })?;
        }
        Ok(expected.clone())
    }
}

fn target_matches(raw: DecodedCallableDefinitionOwner, expected: CallableDefinitionOwner) -> bool {
    use scoop_identity::{
        DecodedStrongCallableDefinitionOwner as D, StrongCallableDefinitionOwner as E,
    };
    match (raw, expected) {
        (
            DecodedCallableDefinitionOwner::Strong(raw),
            CallableDefinitionOwner::Strong(expected),
        ) => match (raw, expected) {
            (D::Function(raw), E::Function(expected)) => raw.verify(expected).is_ok(),
            (D::Constructor(raw), E::Constructor(expected)) => raw.verify(expected).is_ok(),
            (D::PropertyAccessor(raw), E::PropertyAccessor(expected)) => {
                raw.verify(expected).is_ok()
            }
            (D::GeneratedCallable(raw), E::GeneratedCallable(expected)) => {
                raw.verify(expected).is_ok()
            }
            _ => false,
        },
        (DecodedCallableDefinitionOwner::Odr(raw), CallableDefinitionOwner::Odr(expected)) => {
            raw.verify(expected.member()).is_ok()
        }
        _ => false,
    }
}

impl WireDecode for ExactCallableProtocolV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::OrdinaryManaged),
            2 => Ok(Self::OrdinaryNoGc),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

impl WireDecode for DecodedExactCallableAbiExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            target: decoder.field(1, DecodedCallableDefinitionOwner::decode)?,
            signature: decoder.field(2, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            convention: decoder.field(3, crate::CallingConvention::decode)?,
            protocol: decoder.field(4, ExactCallableProtocolV1::decode)?,
            definition: decoder
                .field(6, crate::production::DecodedStrongShapeDefinitionV1::decode)?,
        })
    }
}

impl WireDecode for DecodedCanonicalExactCallableAbiExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactCallableAbiExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedExactCallableAbiExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.convention.encode(encoder)?;
        encoder.field(4)?;
        self.protocol.encode(encoder)?;
        encoder.field(6)?;
        self.definition.encode(encoder)
    }
}

impl WireEncode for DecodedCanonicalExactCallableAbiExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ExactCallableAbiWireError {
    Target,
    Signature,
    Protocol,
    Definition,
    Resource(WireError),
}
impl From<WireError> for ExactCallableAbiWireError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for ExactCallableAbiWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "callable ABI wire differs from checked replay: {self:?}")
    }
}
impl std::error::Error for ExactCallableAbiWireError {}
