//! Strict untrusted wire projection for native contract Code inputs.

use std::fmt;

use scoop_identity::{
    DecodedNativeExternalContract, DecodedNativeExternalSymbolKey, DecodedPersistentId,
    NativeExternalContractFingerprint, PersistentNativeExternalSymbolId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use super::CanonicalNativeExternalContractCodeSetV1;
use crate::{LinkMemberFingerprint, link_object::DecodedFixedBytesV1};

/// Untrusted wire form shared by Link closures that bind the final object set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::link_object) struct DecodedCodeLinkObjectMemberSetV1 {
    members: Vec<DecodedFixedBytesV1<LinkMemberFingerprint>>,
}

impl WireEncode for DecodedCodeLinkObjectMemberSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.members.len() as u64)?;
        for member in &self.members {
            member.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCodeLinkObjectMemberSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedFixedBytesV1::decode(decoder))
            .map(|members| Self { members })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedNativeExternalContractCodeRecordV1 {
    symbol_id: DecodedPersistentId<PersistentNativeExternalSymbolId>,
    symbol_key: DecodedNativeExternalSymbolKey,
    fingerprint: DecodedPersistentId<NativeExternalContractFingerprint>,
    contract: DecodedNativeExternalContract,
}

impl WireEncode for DecodedNativeExternalContractCodeRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(2)?;
        self.symbol_key.encode(encoder)?;
        encoder.field(3)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.contract.encode(encoder)
    }
}

impl WireDecode for DecodedNativeExternalContractCodeRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            symbol_id: decoder.field(1, DecodedPersistentId::decode)?,
            symbol_key: decoder.field(2, DecodedNativeExternalSymbolKey::decode)?,
            fingerprint: decoder.field(3, DecodedPersistentId::decode)?,
            contract: decoder.field(4, DecodedNativeExternalContract::decode)?,
        })
    }
}

/// Canonically decoded native contract Code set without authority to promote
/// identities, symbol keys, or contract references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNativeExternalContractCodeSetV1 {
    contracts: Vec<DecodedNativeExternalContractCodeRecordV1>,
}

impl DecodedCanonicalNativeExternalContractCodeSetV1 {
    /// Promotes only the contract set rebuilt from the verified native
    /// requirement surface after exact canonical equality.
    pub fn validate(
        self,
        expected: &CanonicalNativeExternalContractCodeSetV1,
    ) -> Result<
        CanonicalNativeExternalContractCodeSetV1,
        NativeExternalContractCodeSetValidationError,
    > {
        let actual = encode(&self).map_err(NativeExternalContractCodeSetValidationError::Encode)?;
        let expected_bytes =
            encode(expected).map_err(NativeExternalContractCodeSetValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(NativeExternalContractCodeSetValidationError::ProjectionMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCanonicalNativeExternalContractCodeSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.contracts.len() as u64)?;
        for contract in &self.contracts {
            contract.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalNativeExternalContractCodeSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedNativeExternalContractCodeRecordV1::decode(decoder))
            .map(|contracts| Self { contracts })
    }
}

#[derive(Debug)]
pub enum NativeExternalContractCodeSetValidationError {
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for NativeExternalContractCodeSetValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded native external contract code set: {self:?}"
        )
    }
}

impl std::error::Error for NativeExternalContractCodeSetValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}
