use std::fmt;

use scoop_identity::BackendProfileWireId;
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

const BACKEND_PROFILE_DOMAIN: &str = "scoop-backend-profile-contract-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BackendProfile;

impl BackendProfile {
    pub const LLVM_22_1: Self = Self;

    pub fn wire_id(self) -> BackendProfileWireId {
        BackendProfileWireId::llvm_22_1()
    }

    pub const fn contract(self) -> BackendProfileContract {
        BackendProfileContract
    }

    pub fn fingerprint(self) -> Result<BackendProfileFingerprint, HashError> {
        BackendProfileFingerprint::from_profile(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BackendProfileContract;

impl WireEncode for BackendProfileContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(25)?;
        encode_pair(encoder, 1, 22, 1)?;
        encoder.field(2)?;
        encoder.map(3)?;
        encode_unsigned_field(encoder, 1, 0)?;
        encode_unsigned_field(encoder, 2, 10)?;
        encode_unsigned_field(encoder, 3, 1)?;
        encoder.field(3)?;
        encoder.text("generic")?;
        encoder.field(4)?;
        encoder.text("")?;
        for field in 5..=10 {
            encode_unsigned_field(encoder, field, 1)?;
        }
        encode_unsigned_field(encoder, 11, 3)?;
        for field in 12..=21 {
            encode_unsigned_field(encoder, field, 1)?;
        }
        encode_unsigned_field(encoder, 22, 2)?;
        encoder.field(23)?;
        encoder.map(3)?;
        encode_unsigned_field(encoder, 1, 0xff)?;
        encode_unsigned_field(encoder, 2, 0x9b)?;
        encode_unsigned_field(encoder, 3, 0x01)?;
        encode_unsigned_field(encoder, 24, 1)?;
        encode_unsigned_field(encoder, 25, 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendProfileFingerprint([u8; 32]);

impl BackendProfileFingerprint {
    pub fn from_profile(profile: BackendProfile) -> Result<Self, HashError> {
        let input = BackendProfileFingerprintInput {
            id: profile.wire_id(),
            contract: profile.contract(),
        };
        domain_separated_cbor_hash(BACKEND_PROFILE_DOMAIN, &input)
            .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for BackendProfileFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for BackendProfileFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

struct BackendProfileFingerprintInput {
    id: BackendProfileWireId,
    contract: BackendProfileContract,
}

impl WireEncode for BackendProfileFingerprintInput {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.contract.encode(encoder)
    }
}

fn encode_pair(
    encoder: &mut Encoder,
    field: u32,
    first: u64,
    second: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.map(2)?;
    encode_unsigned_field(encoder, 1, first)?;
    encode_unsigned_field(encoder, 2, second)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::*;

    #[test]
    fn backend_contract_and_fingerprint_match_the_fixed_vectors() {
        assert_eq!(
            hex(&encode(&BackendProfile::LLVM_22_1.contract()).unwrap()),
            "b81901a20116020102a30100020a0301036767656e657269630460050106010701080109010a010b030c010d010e010f01100111011201130114011501160217a30118ff02189b0301181801181901"
        );
        assert_eq!(
            BackendProfile::LLVM_22_1.fingerprint().unwrap().to_string(),
            "03ab3ae611e31f2ac7dde6486deea185c981f5313b939f5cf93641b7e5e9aff7"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
