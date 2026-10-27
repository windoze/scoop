use std::fmt;

use scoop_identity::{BackendProfileId, BackendProfileWireId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

const BACKEND_PROFILE_DOMAIN: &str = "scoop-backend-profile-contract-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BackendProfile {
    id: &'static BackendProfileId,
    contract: &'static BackendProfileContract,
}

impl BackendProfile {
    pub const fn new(
        id: &'static BackendProfileId,
        contract: &'static BackendProfileContract,
    ) -> Self {
        Self { id, contract }
    }

    pub fn wire_id(self) -> BackendProfileWireId {
        BackendProfileWireId::new(*self.id)
    }

    pub const fn contract(self) -> BackendProfileContract {
        *self.contract
    }

    pub fn fingerprint(self) -> Result<BackendProfileFingerprint, HashError> {
        BackendProfileFingerprint::from_profile(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BackendProfileContract {
    pub llvm_api_version: [u32; 2],
    pub inkwell_binding: [u32; 3],
    pub cpu: &'static str,
    pub features: &'static str,
    pub llvm_target_backend: u32,
    pub unwind_provider: u32,
    pub artifact_inspection: crate::NativeObjectFormat,
    pub lsda: BackendLsdaEncodingsV1,
    pub machine_frame: Option<BackendMachineFrameV1>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BackendLsdaEncodingsV1 {
    pub lp_start: u8,
    pub type_table: u8,
    pub call_site: u8,
    pub omitted_type_table: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BackendMachineFrameV1 {
    FixedFrame {
        stack_pointer_register: u16,
        frame_pointer_register: u16,
        root_slot_bytes: u8,
        frame_record_bytes: u8,
    },
    WindowsSeh {
        stack_pointer_register: u16,
        frame_pointer_register: u16,
        root_slot_bytes: u8,
    },
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

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    #[test]
    fn backend_contract_and_fingerprint_match_the_fixed_vectors() {
        assert_eq!(
            hex(&encode(&crate::target_fixtures::LLVM_22_1.contract()).unwrap()),
            "b81a01a20116020102a30100020a0301036767656e657269630460050106010701080109010a010b030c010d010e010f01100111011201130114011501160217a30118ff02189b0301181801181901181c01"
        );
        assert_eq!(
            crate::target_fixtures::LLVM_22_1
                .fingerprint()
                .unwrap()
                .to_string(),
            "08e6c47f288f6522a8987a366d12b110d75ad0090c17b56b184ff70101e95ab8"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

mod encode;
