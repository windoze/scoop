//! Persistent contract for the generated-C producer toolchain.
//!
//! Host compiler and SDK paths are deliberately absent. They are invocation
//! locators and must be validated against this contract before use.

use std::fmt;

use scoop_identity::{CBridgeToolchainProfileId, TargetProfileWireId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{LirTargetProfile, TargetProfileFingerprint};

const C_BRIDGE_TOOLCHAIN_PROFILE_DOMAIN: &str = "scoop-c-bridge-toolchain-profile-v1";
const C_BRIDGE_CANONICAL_FLAGS_DOMAIN: &str = "scoop-c-bridge-canonical-flags-v1";
const GENERATED_C_SOURCE_TEMPLATE_DOMAIN: &str = "scoop-generated-c-source-template-v1";
const MAXIMUM_COMPILER_BUILD_LENGTH: usize = 127;

mod darwin;
mod flags;
mod linux;
mod source;

pub use darwin::{
    AppleClangCompilerIdentityError, AppleClangCompilerIdentityV1, DarwinBuildToolIdV1,
    DarwinBuildToolVersionContractV1, DarwinCBridgeDeploymentContractError,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionError, DarwinPackedVersionV1,
};
pub use flags::{
    CanonicalCBridgeFlagContractV1, CanonicalCBridgeFlagFingerprint, CanonicalCBridgeFlagV1,
};
pub use linux::{GccCompilerIdentityError, GccCompilerIdentityV1};
mod windows;
pub use source::{
    GeneratedCSourceTemplateComponentV1, GeneratedCSourceTemplateContractV1,
    GeneratedCSourceTemplateFingerprint,
};
pub use windows::WindowsClangCompilerIdentityV1;

/// The generated-C compiler inherits no host environment variables.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CBridgeEnvironmentProjectionV1;

impl CBridgeEnvironmentProjectionV1 {
    pub const CLEAN_C_LOCALE_UTC: Self = Self;

    pub const fn locale(self) -> &'static str {
        "C"
    }

    pub const fn timezone(self) -> &'static str {
        "UTC"
    }
}

impl WireEncode for CBridgeEnvironmentProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.array(0)?;
        encoder.field(2)?;
        encoder.text(self.locale())?;
        encoder.field(3)?;
        encoder.text(self.timezone())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CBridgePlatformContractV1 {
    Darwin {
        deployment: DarwinCBridgeDeploymentContractV1,
        compiler: AppleClangCompilerIdentityV1,
    },
    Linux {
        compiler: GccCompilerIdentityV1,
    },
    Windows {
        compiler: WindowsClangCompilerIdentityV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CBridgePlatformError {
    ExpectedDarwin,
    ExpectedLinux,
    ExpectedWindows,
}

impl fmt::Display for CBridgePlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedDarwin => "a Darwin C toolchain is required for Mach-O deployment",
            Self::ExpectedLinux => "a Linux target is required for the GCC C toolchain",
            Self::ExpectedWindows => "a Windows target is required for the Windows C toolchain",
        })
    }
}
impl std::error::Error for CBridgePlatformError {}

#[derive(Debug)]
pub enum CBridgeToolchainBuildError {
    Platform(CBridgePlatformError),
    Hash(HashError),
}
impl fmt::Display for CBridgeToolchainBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for CBridgeToolchainBuildError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CBridgeToolchainContractV1 {
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    canonical_triple: &'static str,
    platform: CBridgePlatformContractV1,
    canonical_flag_fingerprint: CanonicalCBridgeFlagFingerprint,
    source_template_fingerprint: GeneratedCSourceTemplateFingerprint,
    environment: CBridgeEnvironmentProjectionV1,
}

impl CBridgeToolchainContractV1 {
    pub fn new(
        target: LirTargetProfile,
        platform: CBridgePlatformContractV1,
        flags: CanonicalCBridgeFlagContractV1,
    ) -> Result<Self, HashError> {
        Ok(Self {
            target: target.wire_id(),
            target_fingerprint: target.fingerprint()?,
            canonical_triple: target.contract().canonical_triple(),
            platform,
            canonical_flag_fingerprint: flags.fingerprint()?,
            source_template_fingerprint: GeneratedCSourceTemplateContractV1::CURRENT
                .fingerprint()?,
            environment: CBridgeEnvironmentProjectionV1::CLEAN_C_LOCALE_UTC,
        })
    }

    pub const fn target(&self) -> &TargetProfileWireId {
        &self.target
    }

    pub const fn target_fingerprint(&self) -> TargetProfileFingerprint {
        self.target_fingerprint
    }

    pub const fn canonical_triple(&self) -> &'static str {
        self.canonical_triple
    }

    pub const fn platform(&self) -> &CBridgePlatformContractV1 {
        &self.platform
    }

    pub fn deployment(&self) -> Result<&DarwinCBridgeDeploymentContractV1, CBridgePlatformError> {
        match &self.platform {
            CBridgePlatformContractV1::Darwin { deployment, .. } => Ok(deployment),
            CBridgePlatformContractV1::Linux { .. } | CBridgePlatformContractV1::Windows { .. } => {
                Err(CBridgePlatformError::ExpectedDarwin)
            }
        }
    }

    pub fn compiler(&self) -> Result<&AppleClangCompilerIdentityV1, CBridgePlatformError> {
        match &self.platform {
            CBridgePlatformContractV1::Darwin { compiler, .. } => Ok(compiler),
            CBridgePlatformContractV1::Linux { .. } | CBridgePlatformContractV1::Windows { .. } => {
                Err(CBridgePlatformError::ExpectedDarwin)
            }
        }
    }

    pub const fn canonical_flag_fingerprint(&self) -> CanonicalCBridgeFlagFingerprint {
        self.canonical_flag_fingerprint
    }

    pub const fn source_template_fingerprint(&self) -> GeneratedCSourceTemplateFingerprint {
        self.source_template_fingerprint
    }

    pub const fn environment(&self) -> CBridgeEnvironmentProjectionV1 {
        self.environment
    }
}

impl WireEncode for CBridgeToolchainContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        encoder.text(self.canonical_triple)?;
        encoder.field(4)?;
        match &self.platform {
            CBridgePlatformContractV1::Darwin {
                deployment,
                compiler,
            } => {
                deployment.encode(encoder)?;
                encoder.field(5)?;
                compiler.encode(encoder)?;
            }
            CBridgePlatformContractV1::Linux { compiler } => {
                encoder.map(1)?;
                encoder.field(1)?;
                encoder.unsigned(2)?;
                encoder.field(5)?;
                compiler.encode(encoder)?;
            }
            CBridgePlatformContractV1::Windows { compiler } => {
                encoder.map(1)?;
                encoder.field(1)?;
                encoder.unsigned(3)?;
                encoder.field(5)?;
                compiler.encode(encoder)?;
            }
        }
        encoder.field(6)?;
        self.canonical_flag_fingerprint.encode(encoder)?;
        encoder.field(7)?;
        self.source_template_fingerprint.encode(encoder)?;
        encoder.field(8)?;
        self.environment.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CBridgeToolchainProfileV1 {
    id: CBridgeToolchainProfileId,
    contract: CBridgeToolchainContractV1,
    fingerprint: CBridgeToolchainFingerprint,
}

impl CBridgeToolchainProfileV1 {
    pub fn new(
        id: CBridgeToolchainProfileId,
        contract: CBridgeToolchainContractV1,
    ) -> Result<Self, HashError> {
        let fingerprint = CBridgeToolchainFingerprint::from_parts(&id, &contract)?;
        Ok(Self {
            id,
            contract,
            fingerprint,
        })
    }

    pub const fn id(&self) -> &CBridgeToolchainProfileId {
        &self.id
    }

    pub const fn contract(&self) -> &CBridgeToolchainContractV1 {
        &self.contract
    }

    pub const fn fingerprint(&self) -> CBridgeToolchainFingerprint {
        self.fingerprint
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CBridgeToolchainFingerprint([u8; 32]);

impl CBridgeToolchainFingerprint {
    fn from_parts(
        id: &CBridgeToolchainProfileId,
        contract: &CBridgeToolchainContractV1,
    ) -> Result<Self, HashError> {
        domain_separated_cbor_hash(
            C_BRIDGE_TOOLCHAIN_PROFILE_DOMAIN,
            &CBridgeToolchainFingerprintInput { id, contract },
        )
        .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CBridgeToolchainFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CBridgeToolchainFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

struct CBridgeToolchainFingerprintInput<'a> {
    id: &'a CBridgeToolchainProfileId,
    contract: &'a CBridgeToolchainContractV1,
}

impl WireEncode for CBridgeToolchainFingerprintInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.contract.encode(encoder)
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

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(u64::from(value))
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8; 32]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
