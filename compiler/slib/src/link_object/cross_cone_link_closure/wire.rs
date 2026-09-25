//! Strict untrusted wire form of the cross-Cone Link closure.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedCanonicalScoopAbiFunctionSignature, DecodedPersistentId,
    DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner, ObjectDefinitionPlanId,
};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encode,
    encode_canonical_temporary_with_meter,
};

use super::{
    CrossConeLinkClosureSectionV1, CrossConeLinkSemanticImportSetV1,
    CrossConeRelocationUseSetDigestV1,
};
use crate::link_object::{
    DecodedCanonicalUndefinedRelocationUseV1, DecodedCodeLinkObjectMemberSetV1, DecodedFixedBytesV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCrossConeLinkSemanticImportV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedCrossConeLinkSemanticImportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(4)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(5)?;
        self.required_definition.encode(encoder)
    }
}

impl WireDecode for DecodedCrossConeLinkSemanticImportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            abi_signature: decoder.field(3, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            expected_symbol: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
            required_definition: decoder.field(5, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCrossConeLinkSemanticImportSetV1 {
    imports: Vec<DecodedCrossConeLinkSemanticImportV1>,
}

impl DecodedCrossConeLinkSemanticImportSetV1 {
    fn validate_against(
        &self,
        expected: &CrossConeLinkSemanticImportSetV1,
    ) -> Result<(), CrossConeLinkClosureSectionValidationError> {
        let actual = encode(self).map_err(CrossConeLinkClosureSectionValidationError::Encoding)?;
        let expected =
            encode(expected).map_err(CrossConeLinkClosureSectionValidationError::Encoding)?;
        if actual != expected {
            return Err(CrossConeLinkClosureSectionValidationError::SemanticProjectionMismatch);
        }
        Ok(())
    }
}

impl WireEncode for DecodedCrossConeLinkSemanticImportSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.imports)
    }
}

impl WireDecode for DecodedCrossConeLinkSemanticImportSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCrossConeLinkSemanticImportV1::decode(decoder))
            .map(|imports| Self { imports })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCrossConeUndefinedRequirementV1 {
    use_site: DecodedCanonicalUndefinedRelocationUseV1,
    import_index: u32,
}

impl WireEncode for DecodedCrossConeUndefinedRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.import_index))
    }
}

impl WireDecode for DecodedCrossConeUndefinedRequirementV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            use_site: decoder.field(1, DecodedCanonicalUndefinedRelocationUseV1::decode)?,
            import_index: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCrossConeObjectCoverageProofV1 {
    verified_link_objects: DecodedCodeLinkObjectMemberSetV1,
    relocation_use_set_digest: DecodedFixedBytesV1<CrossConeRelocationUseSetDigestV1>,
}

impl WireEncode for DecodedCrossConeObjectCoverageProofV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(2)?;
        self.relocation_use_set_digest.encode(encoder)
    }
}

impl WireDecode for DecodedCrossConeObjectCoverageProofV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            verified_link_objects: decoder.field(1, DecodedCodeLinkObjectMemberSetV1::decode)?,
            relocation_use_set_digest: decoder.field(2, DecodedFixedBytesV1::decode)?,
        })
    }
}

/// Decoded closure data carries no semantic or physical authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeLinkClosureSectionV1 {
    semantic_imports: DecodedCrossConeLinkSemanticImportSetV1,
    requirements: Vec<DecodedCrossConeUndefinedRequirementV1>,
    object_coverage: DecodedCrossConeObjectCoverageProofV1,
}

impl DecodedCrossConeLinkClosureSectionV1 {
    /// Replays the semantic and actual-use projections while retaining the
    /// original coverage fields for final object and Code verification.
    pub fn replay_requirements_against(
        &self,
        expected: &crate::VerifiedCrossConeStrongRequirementClosureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), CrossConeLinkClosureSectionValidationError> {
        if !same_bytes(
            &self.semantic_imports,
            expected.semantic_imports(),
            1,
            meter,
        )? {
            return Err(CrossConeLinkClosureSectionValidationError::SemanticProjectionMismatch);
        }
        if !same_bytes(
            &WireArray(&self.requirements),
            &WireArray(expected.requirements()),
            2,
            meter,
        )? {
            return Err(CrossConeLinkClosureSectionValidationError::ProjectionMismatch);
        }
        Ok(())
    }

    /// Checks the Code projection before object validation has completed.
    pub fn validate_semantic_imports_against(
        &self,
        expected: &CrossConeLinkSemanticImportSetV1,
    ) -> Result<(), CrossConeLinkClosureSectionValidationError> {
        self.semantic_imports.validate_against(expected)
    }

    /// Promotes only a section independently rebuilt from verified objects.
    pub fn validate_against(
        self,
        expected: &CrossConeLinkClosureSectionV1,
    ) -> Result<CrossConeLinkClosureSectionV1, CrossConeLinkClosureSectionValidationError> {
        let actual = encode(&self).map_err(CrossConeLinkClosureSectionValidationError::Encoding)?;
        let expected_bytes =
            encode(expected).map_err(CrossConeLinkClosureSectionValidationError::Encoding)?;
        if actual != expected_bytes {
            return Err(CrossConeLinkClosureSectionValidationError::ProjectionMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCrossConeLinkClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_imports.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.requirements)?;
        encoder.field(3)?;
        self.object_coverage.encode(encoder)
    }
}

impl WireDecode for DecodedCrossConeLinkClosureSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            semantic_imports: decoder.field(1, DecodedCrossConeLinkSemanticImportSetV1::decode)?,
            requirements: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedCrossConeUndefinedRequirementV1::decode(decoder)
                })
            })?,
            object_coverage: decoder.field(3, DecodedCrossConeObjectCoverageProofV1::decode)?,
        })
    }
}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

struct WireArray<'a, T>(&'a [T]);

impl<T: WireEncode> WireEncode for WireArray<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, self.0)
    }
}

fn same_bytes(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    field: u32,
    meter: &mut BudgetMeter,
) -> Result<bool, CrossConeLinkClosureSectionValidationError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    Ok(actual == expected)
}

impl From<WireError> for CrossConeLinkClosureSectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Debug)]
pub enum CrossConeLinkClosureSectionValidationError {
    Resource(WireError),
    SemanticProjectionMismatch,
    ProjectionMismatch,
    Encoding(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for CrossConeLinkClosureSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded cross-Cone Link closure: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLinkClosureSectionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Encoding(source) => Some(source),
            Self::SemanticProjectionMismatch | Self::ProjectionMismatch => None,
        }
    }
}
