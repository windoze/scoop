use std::fmt;

use scoop_identity::{
    ArtifactCapabilityProfileId, BackendProfileWireId, ManglingSchemaIdentity, TargetProfileWireId,
};
use scoop_lir::{BackendProfileFingerprint, TargetProfileFingerprint, ValidatedLirTargetSelection};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{ArtifactCapabilityProfile, ArtifactCapabilityProfileFingerprint};

const LANGUAGE_ABI_DOMAIN: &str = "scoop-language-abi-contract-v1";
const RUNTIME_ABI_DOMAIN: &str = "scoop-runtime-abi-contract-v1";
const COMPOSITE_IDENTITY_ABI_DOMAIN: &str = "scoop-composite-identity-abi-v1";
const INITIAL_SCHEMA: u64 = 1;

macro_rules! typed_fingerprint {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

typed_fingerprint!(LanguageAbiFingerprint);
typed_fingerprint!(RuntimeAbiFingerprint);
typed_fingerprint!(CompositeIdentityAbiFingerprint);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LanguageAbiContract;

impl LanguageAbiContract {
    pub fn fingerprint(self) -> Result<LanguageAbiFingerprint, HashError> {
        domain_separated_cbor_hash(LANGUAGE_ABI_DOMAIN, &self)
            .map(|digest| LanguageAbiFingerprint(*digest.as_array()))
    }
}

impl WireEncode for LanguageAbiContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.unsigned(INITIAL_SCHEMA)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeAbiContract;

impl RuntimeAbiContract {
    pub fn fingerprint(self) -> Result<RuntimeAbiFingerprint, HashError> {
        domain_separated_cbor_hash(RUNTIME_ABI_DOMAIN, &self)
            .map(|digest| RuntimeAbiFingerprint(*digest.as_array()))
    }
}

impl WireEncode for RuntimeAbiContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        for field in 1..=3 {
            encoder.field(field)?;
            encoder.unsigned(INITIAL_SCHEMA)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IdentityAbiDescriptor {
    language_abi: LanguageAbiFingerprint,
    runtime_abi: RuntimeAbiFingerprint,
    mangling_schema: ManglingSchemaIdentity,
}

impl IdentityAbiDescriptor {
    pub fn current() -> Result<Self, HashError> {
        Ok(Self {
            language_abi: LanguageAbiContract.fingerprint()?,
            runtime_abi: RuntimeAbiContract.fingerprint()?,
            mangling_schema: ManglingSchemaIdentity,
        })
    }

    pub const fn language_abi(self) -> LanguageAbiFingerprint {
        self.language_abi
    }

    pub const fn runtime_abi(self) -> RuntimeAbiFingerprint {
        self.runtime_abi
    }

    pub const fn mangling_schema(self) -> ManglingSchemaIdentity {
        self.mangling_schema
    }

    pub fn fingerprint(self) -> Result<CompositeIdentityAbiFingerprint, HashError> {
        domain_separated_cbor_hash(COMPOSITE_IDENTITY_ABI_DOMAIN, &self)
            .map(|digest| CompositeIdentityAbiFingerprint(*digest.as_array()))
    }
}

impl WireEncode for IdentityAbiDescriptor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(13)?;
        encoder.field(1)?;
        self.language_abi.encode(encoder)?;
        encoder.field(2)?;
        self.runtime_abi.encode(encoder)?;
        for field in 3..=6 {
            encode_initial_schema(encoder, field)?;
        }
        encoder.field(7)?;
        self.mangling_schema.encode(encoder)?;
        for field in 8..=13 {
            encode_initial_schema(encoder, field)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityRecord {
    language_abi: LanguageAbiFingerprint,
    runtime_abi: RuntimeAbiFingerprint,
    mangling_schema: ManglingSchemaIdentity,
    target_profile: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    backend_profile: BackendProfileWireId,
    backend_fingerprint: BackendProfileFingerprint,
    composite_identity_abi: CompositeIdentityAbiFingerprint,
    artifact_profile: ArtifactCapabilityProfileId,
    artifact_profile_fingerprint: ArtifactCapabilityProfileFingerprint,
}

impl CompatibilityRecord {
    pub fn identity_foundation(selection: ValidatedLirTargetSelection) -> Result<Self, HashError> {
        let descriptor = IdentityAbiDescriptor::current()?;
        let target = selection.target();
        let backend = selection.backend();
        let artifact_profile = ArtifactCapabilityProfile::IDENTITY_FOUNDATION;
        Ok(Self {
            language_abi: descriptor.language_abi(),
            runtime_abi: descriptor.runtime_abi(),
            mangling_schema: descriptor.mangling_schema(),
            target_profile: target.wire_id(),
            target_fingerprint: target.fingerprint()?,
            backend_profile: backend.wire_id(),
            backend_fingerprint: backend.fingerprint()?,
            composite_identity_abi: descriptor.fingerprint()?,
            artifact_profile: artifact_profile.id(),
            artifact_profile_fingerprint: artifact_profile.fingerprint()?,
        })
    }

    pub const fn language_abi(&self) -> LanguageAbiFingerprint {
        self.language_abi
    }

    pub const fn runtime_abi(&self) -> RuntimeAbiFingerprint {
        self.runtime_abi
    }

    pub const fn target_fingerprint(&self) -> TargetProfileFingerprint {
        self.target_fingerprint
    }

    pub const fn backend_fingerprint(&self) -> BackendProfileFingerprint {
        self.backend_fingerprint
    }

    pub const fn composite_identity_abi(&self) -> CompositeIdentityAbiFingerprint {
        self.composite_identity_abi
    }

    pub const fn artifact_profile_fingerprint(&self) -> ArtifactCapabilityProfileFingerprint {
        self.artifact_profile_fingerprint
    }
}

impl WireEncode for CompatibilityRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(14)?;
        encoder.field(1)?;
        self.language_abi.encode(encoder)?;
        encoder.field(2)?;
        self.runtime_abi.encode(encoder)?;
        encode_initial_schema(encoder, 3)?;
        encoder.field(4)?;
        self.mangling_schema.encode(encoder)?;
        encoder.field(5)?;
        self.target_profile.encode(encoder)?;
        encoder.field(6)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(7)?;
        self.backend_profile.encode(encoder)?;
        encoder.field(8)?;
        self.backend_fingerprint.encode(encoder)?;
        for field in 9..=11 {
            encode_initial_schema(encoder, field)?;
        }
        encoder.field(12)?;
        self.composite_identity_abi.encode(encoder)?;
        encoder.field(13)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(14)?;
        self.artifact_profile_fingerprint.encode(encoder)
    }
}

fn encode_initial_schema(
    encoder: &mut Encoder,
    field: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(INITIAL_SCHEMA)
}

#[cfg(test)]
mod tests {
    use scoop_lir::ValidatedLirTargetSelection;
    use scoop_wire::encode;

    use super::*;

    #[test]
    fn language_runtime_and_composite_abi_have_fixed_vectors() {
        assert_eq!(hex(&encode(&LanguageAbiContract).unwrap()), "a10101");
        assert_eq!(
            LanguageAbiContract.fingerprint().unwrap().to_string(),
            "2d2188ce81a619a325eeb6b4475bd4ac1dc0a220e463f3858e58f26ef21c9c96"
        );
        assert_eq!(hex(&encode(&RuntimeAbiContract).unwrap()), "a3010102010301");
        assert_eq!(
            RuntimeAbiContract.fingerprint().unwrap().to_string(),
            "72be5e11c123875bccf1dcad1d91fd5ae870e076ae91c1c2a264a3dcc0e56a8e"
        );

        let descriptor = IdentityAbiDescriptor::current().unwrap();
        assert_eq!(
            hex(&encode(&descriptor).unwrap()),
            "ad0158202d2188ce81a619a325eeb6b4475bd4ac1dc0a220e463f3858e58f26ef21c9c9602582072be5e11c123875bccf1dcad1d91fd5ae870e076ae91c1c2a264a3dcc0e56a8e0301040105010601076d70657273697374656e742d7631080109010a010b010c010d01"
        );
        assert_eq!(
            descriptor.fingerprint().unwrap().to_string(),
            "f4e2a5c02cedc7b713a65e4165d3b8182fe831237ee2e00d540ca07879f042e2"
        );
    }

    #[test]
    fn compatibility_record_is_derived_from_the_only_registered_contracts() {
        let record = CompatibilityRecord::identity_foundation(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        let encoded = encode(&record).unwrap();

        assert_eq!(encoded[0], 0xae);
        assert_eq!(
            record.target_fingerprint().to_string(),
            "42697b4e4e2102ef19d81bd624f7e428065d2ddff90279f1fc458bfcfdb13671"
        );
        assert_eq!(
            record.backend_fingerprint().to_string(),
            "03ab3ae611e31f2ac7dde6486deea185c981f5313b939f5cf93641b7e5e9aff7"
        );
        assert_eq!(
            record.artifact_profile_fingerprint().to_string(),
            "6461601c81a77ecbd34698c708d9035732f0a75f6e5ec98d2c561250ab0111f9"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
