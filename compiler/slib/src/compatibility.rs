use std::fmt;

use scoop_identity::{
    ArtifactCapabilityProfileId, BackendProfileWireId, ManglingSchemaIdentity, TargetProfileWireId,
};
use scoop_lir::{
    BackendProfileFingerprint, RuntimeAbiContract, RuntimeAbiFingerprint, TargetProfileFingerprint,
    ValidatedLirTargetSelection,
};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{ArtifactCapabilityProfile, ArtifactCapabilityProfileFingerprint};

const LANGUAGE_ABI_DOMAIN: &str = "scoop-language-abi-contract-v1";
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
        encoder.unsigned(2)
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
        for field in 3..=5 {
            encode_initial_schema(encoder, field)?;
        }
        encoder.field(6)?;
        encoder.unsigned(2)?;
        encoder.field(7)?;
        self.mangling_schema.encode(encoder)?;
        for field in 8..=10 {
            encode_initial_schema(encoder, field)?;
        }
        for field in 11..=13 {
            encoder.field(field)?;
            encoder.unsigned(2)?;
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
    pub fn new(
        selection: ValidatedLirTargetSelection,
        artifact_profile: ArtifactCapabilityProfile,
    ) -> Result<Self, HashError> {
        let descriptor = IdentityAbiDescriptor::current()?;
        let target = selection.target();
        let backend = selection.backend();
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

    pub const fn mangling_schema(&self) -> ManglingSchemaIdentity {
        self.mangling_schema
    }

    pub const fn target_profile(&self) -> &TargetProfileWireId {
        &self.target_profile
    }

    pub const fn target_fingerprint(&self) -> TargetProfileFingerprint {
        self.target_fingerprint
    }

    pub const fn backend_profile(&self) -> &BackendProfileWireId {
        &self.backend_profile
    }

    pub const fn backend_fingerprint(&self) -> BackendProfileFingerprint {
        self.backend_fingerprint
    }

    pub const fn composite_identity_abi(&self) -> CompositeIdentityAbiFingerprint {
        self.composite_identity_abi
    }

    pub const fn artifact_profile(&self) -> &ArtifactCapabilityProfileId {
        &self.artifact_profile
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
            encoder.field(field)?;
            encoder.unsigned(u64::from(crate::metadata::METADATA_SCHEMA))?;
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
        assert_eq!(hex(&encode(&LanguageAbiContract).unwrap()), "a10102");
        assert_eq!(
            LanguageAbiContract.fingerprint().unwrap().to_string(),
            "634ec02192ba1541f603b8b56f8c9e63dfc31d86ca5d4443a626f6afc9005391"
        );
        assert_eq!(hex(&encode(&RuntimeAbiContract).unwrap()), "a3010902010302");
        assert_eq!(
            RuntimeAbiContract.fingerprint().unwrap().to_string(),
            "84066a1e28c42a6f63d910e884d82896f178b4b01c1e9a9638f47537526b7f24"
        );

        let descriptor = IdentityAbiDescriptor::current().unwrap();
        assert_eq!(
            hex(&encode(&descriptor).unwrap()),
            "ad015820634ec02192ba1541f603b8b56f8c9e63dfc31d86ca5d4443a626f6afc900539102582084066a1e28c42a6f63d910e884d82896f178b4b01c1e9a9638f47537526b7f240301040105010602076d70657273697374656e742d7631080109010a010b020c020d02"
        );
        assert_eq!(
            descriptor.fingerprint().unwrap().to_string(),
            "4aabba581b4ca4fcffb4e3e020923e507a1e75b4aca4ed3bdd1bc88a89cb3509"
        );
    }

    #[test]
    fn compatibility_record_is_derived_from_the_only_registered_contracts() {
        let record = CompatibilityRecord::new(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
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
            "97adff024fd1bac2b70e7293eccc178a6e064e4be672f4dd23859bf1ce3eeeff"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
