//! Persistent contract for the generated-C producer toolchain.
//!
//! Host compiler and SDK paths are deliberately absent. They are invocation
//! locators and must be validated against this contract before use.

use std::fmt;

use scoop_identity::{CBridgeToolchainProfileId, TargetProfileWireId};
use scoop_wire::{
    Encoder, HashError, WireEncode, domain_separated_cbor_hash,
    domain_separated_cbor_hash_stream_length,
};

use crate::{LirTargetProfile, TargetProfileFingerprint};

const C_BRIDGE_TOOLCHAIN_PROFILE_DOMAIN: &str = "scoop-c-bridge-toolchain-profile-v1";
const C_BRIDGE_CANONICAL_FLAGS_DOMAIN: &str = "scoop-c-bridge-canonical-flags-v1";
const GENERATED_C_SOURCE_TEMPLATE_DOMAIN: &str = "scoop-generated-c-source-template-v1";
const MAXIMUM_COMPILER_BUILD_LENGTH: usize = 127;

/// Darwin's packed `X.Y.Z` version used by `LC_BUILD_VERSION`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DarwinPackedVersionV1(u32);

impl DarwinPackedVersionV1 {
    pub fn new(packed: u32) -> Result<Self, DarwinPackedVersionError> {
        if packed == 0 {
            return Err(DarwinPackedVersionError::Zero);
        }
        Ok(Self(packed))
    }

    pub fn from_components(
        major: u32,
        minor: u32,
        patch: u32,
    ) -> Result<Self, DarwinPackedVersionError> {
        if major == 0 {
            return Err(DarwinPackedVersionError::Zero);
        }
        if major > u32::from(u16::MAX) {
            return Err(DarwinPackedVersionError::MajorOutOfRange(major));
        }
        if minor > u32::from(u8::MAX) {
            return Err(DarwinPackedVersionError::MinorOutOfRange(minor));
        }
        if patch > u32::from(u8::MAX) {
            return Err(DarwinPackedVersionError::PatchOutOfRange(patch));
        }
        Self::new((major << 16) | (minor << 8) | patch)
    }

    pub const fn packed(self) -> u32 {
        self.0
    }

    pub const fn components(self) -> (u32, u32, u32) {
        (self.0 >> 16, (self.0 >> 8) & 0xff, self.0 & 0xff)
    }
}

impl WireEncode for DarwinPackedVersionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinPackedVersionError {
    Zero,
    MajorOutOfRange(u32),
    MinorOutOfRange(u32),
    PatchOutOfRange(u32),
}

impl fmt::Display for DarwinPackedVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Darwin packed version: {self:?}")
    }
}

impl std::error::Error for DarwinPackedVersionError {}

impl fmt::Display for DarwinPackedVersionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (major, minor, patch) = self.components();
        write!(formatter, "{major}.{minor}.{patch}")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinBuildToolIdV1 {
    Clang,
    Ld,
    Lld,
}

impl DarwinBuildToolIdV1 {
    pub const fn macho_value(self) -> u32 {
        match self {
            Self::Clang => 1,
            Self::Ld => 3,
            Self::Lld => 4,
        }
    }
}

impl WireEncode for DarwinBuildToolIdV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.macho_value()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DarwinBuildToolVersionContractV1 {
    tool: DarwinBuildToolIdV1,
    version: DarwinPackedVersionV1,
}

impl DarwinBuildToolVersionContractV1 {
    pub const fn new(tool: DarwinBuildToolIdV1, version: DarwinPackedVersionV1) -> Self {
        Self { tool, version }
    }

    pub const fn tool(self) -> DarwinBuildToolIdV1 {
        self.tool
    }

    pub const fn version(self) -> DarwinPackedVersionV1 {
        self.version
    }
}

impl WireEncode for DarwinBuildToolVersionContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.tool.encode(encoder)?;
        encoder.field(2)?;
        self.version.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DarwinCBridgeDeploymentContractV1 {
    minimum_os: DarwinPackedVersionV1,
    sdk: DarwinPackedVersionV1,
    tools: Vec<DarwinBuildToolVersionContractV1>,
}

impl DarwinCBridgeDeploymentContractV1 {
    pub fn new(
        minimum_os: DarwinPackedVersionV1,
        sdk: DarwinPackedVersionV1,
        tools: Vec<DarwinBuildToolVersionContractV1>,
    ) -> Result<Self, DarwinCBridgeDeploymentContractError> {
        for (index, tool) in tools.iter().enumerate().skip(1) {
            let previous = tools[index - 1].tool();
            if previous >= tool.tool() {
                return Err(if previous == tool.tool() {
                    DarwinCBridgeDeploymentContractError::DuplicateTool(tool.tool())
                } else {
                    DarwinCBridgeDeploymentContractError::NonCanonicalToolOrder { index }
                });
            }
        }
        if !tools.is_empty()
            && !tools
                .iter()
                .any(|tool| tool.tool() == DarwinBuildToolIdV1::Clang)
        {
            return Err(DarwinCBridgeDeploymentContractError::MissingClang);
        }
        Ok(Self {
            minimum_os,
            sdk,
            tools,
        })
    }

    pub const fn minimum_os(&self) -> DarwinPackedVersionV1 {
        self.minimum_os
    }

    pub const fn sdk(&self) -> DarwinPackedVersionV1 {
        self.sdk
    }

    pub fn tools(&self) -> &[DarwinBuildToolVersionContractV1] {
        &self.tools
    }
}

impl WireEncode for DarwinCBridgeDeploymentContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.unsigned(1)?;
        encoder.field(2)?;
        self.minimum_os.encode(encoder)?;
        encoder.field(3)?;
        self.sdk.encode(encoder)?;
        encoder.field(4)?;
        encode_array(encoder, &self.tools)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinCBridgeDeploymentContractError {
    MissingClang,
    DuplicateTool(DarwinBuildToolIdV1),
    NonCanonicalToolOrder { index: usize },
}

impl fmt::Display for DarwinCBridgeDeploymentContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin generated-C deployment contract: {self:?}"
        )
    }
}

impl std::error::Error for DarwinCBridgeDeploymentContractError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AppleClangCompilerIdentityV1 {
    version_major: u32,
    version_minor: u32,
    version_patch: u32,
    build: String,
}

impl AppleClangCompilerIdentityV1 {
    pub fn new(
        version_major: u32,
        version_minor: u32,
        version_patch: u32,
        build: &str,
    ) -> Result<Self, AppleClangCompilerIdentityError> {
        if version_major == 0 {
            return Err(AppleClangCompilerIdentityError::ZeroMajorVersion);
        }
        if build.is_empty() {
            return Err(AppleClangCompilerIdentityError::EmptyBuild);
        }
        if build.len() > MAXIMUM_COMPILER_BUILD_LENGTH {
            return Err(AppleClangCompilerIdentityError::BuildTooLong);
        }
        if !build
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
        {
            return Err(AppleClangCompilerIdentityError::InvalidBuildCharacter);
        }
        Ok(Self {
            version_major,
            version_minor,
            version_patch,
            build: build.to_owned(),
        })
    }

    pub const fn version_major(&self) -> u32 {
        self.version_major
    }

    pub const fn version_minor(&self) -> u32 {
        self.version_minor
    }

    pub const fn version_patch(&self) -> u32 {
        self.version_patch
    }

    pub fn build(&self) -> &str {
        &self.build
    }
}

impl WireEncode for AppleClangCompilerIdentityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encode_unsigned_field(encoder, 1, self.version_major)?;
        encode_unsigned_field(encoder, 2, self.version_minor)?;
        encode_unsigned_field(encoder, 3, self.version_patch)?;
        encoder.field(4)?;
        encoder.text(&self.build)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleClangCompilerIdentityError {
    ZeroMajorVersion,
    EmptyBuild,
    BuildTooLong,
    InvalidBuildCharacter,
}

impl fmt::Display for AppleClangCompilerIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Apple Clang compiler identity: {self:?}")
    }
}

impl std::error::Error for AppleClangCompilerIdentityError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalCBridgeFlagContractV1;

impl CanonicalCBridgeFlagContractV1 {
    pub const CURRENT: Self = Self;

    pub const fn flags(self) -> &'static [CanonicalCBridgeFlagV1] {
        &CanonicalCBridgeFlagV1::ALL
    }

    pub fn fingerprint(self) -> Result<CanonicalCBridgeFlagFingerprint, HashError> {
        domain_separated_cbor_hash(C_BRIDGE_CANONICAL_FLAGS_DOMAIN, &self)
            .map(|digest| CanonicalCBridgeFlagFingerprint(*digest.as_array()))
    }
}

impl WireEncode for CanonicalCBridgeFlagContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(CanonicalCBridgeFlagV1::ALL.len() as u64)?;
        for flag in CanonicalCBridgeFlagV1::ALL {
            flag.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanonicalCBridgeFlagV1 {
    ExplicitCanonicalTarget,
    ExplicitResolvedSdkRoot,
    ExplicitMinimumDeployment,
    C11,
    RelocatableObject,
    Unoptimized,
    NoDebugInformation,
    NoCommonSymbols,
    OmitCompilerIdentification,
    NoStackProtector,
    NoUnwindTables,
    NoAsynchronousUnwindTables,
}

impl CanonicalCBridgeFlagV1 {
    pub const ALL: [Self; 12] = [
        Self::ExplicitCanonicalTarget,
        Self::ExplicitResolvedSdkRoot,
        Self::ExplicitMinimumDeployment,
        Self::C11,
        Self::RelocatableObject,
        Self::Unoptimized,
        Self::NoDebugInformation,
        Self::NoCommonSymbols,
        Self::OmitCompilerIdentification,
        Self::NoStackProtector,
        Self::NoUnwindTables,
        Self::NoAsynchronousUnwindTables,
    ];

    const fn tag(self) -> u32 {
        match self {
            Self::ExplicitCanonicalTarget => 1,
            Self::ExplicitResolvedSdkRoot => 2,
            Self::ExplicitMinimumDeployment => 3,
            Self::C11 => 4,
            Self::RelocatableObject => 5,
            Self::Unoptimized => 6,
            Self::NoDebugInformation => 7,
            Self::NoCommonSymbols => 8,
            Self::OmitCompilerIdentification => 9,
            Self::NoStackProtector => 10,
            Self::NoUnwindTables => 11,
            Self::NoAsynchronousUnwindTables => 12,
        }
    }
}

impl WireEncode for CanonicalCBridgeFlagV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCBridgeFlagFingerprint([u8; 32]);

impl CanonicalCBridgeFlagFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CanonicalCBridgeFlagFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CanonicalCBridgeFlagFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeneratedCSourceTemplateContractV1;

impl GeneratedCSourceTemplateContractV1 {
    pub const CURRENT: Self = Self;

    pub const fn components(self) -> &'static [GeneratedCSourceTemplateComponentV1] {
        &GeneratedCSourceTemplateComponentV1::ALL
    }

    pub fn fingerprint(self) -> Result<GeneratedCSourceTemplateFingerprint, HashError> {
        domain_separated_cbor_hash(GENERATED_C_SOURCE_TEMPLATE_DOMAIN, &self)
            .map(|digest| GeneratedCSourceTemplateFingerprint(*digest.as_array()))
    }
}

impl WireEncode for GeneratedCSourceTemplateContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(GeneratedCSourceTemplateComponentV1::ALL.len() as u64)?;
        for component in GeneratedCSourceTemplateComponentV1::ALL {
            encoder.map(2)?;
            encoder.field(1)?;
            component.encode(encoder)?;
            encode_unsigned_field(encoder, 2, 1)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneratedCSourceTemplateComponentV1 {
    TypeRenderer,
    LayoutAssertions,
    ExternalDeclarations,
    OutboundWrappers,
    NativeGlobalAccessors,
    CallbackTrampolines,
    ForeignCallbackTrampolines,
    UnitObjectPartition,
    AtomBoundaryMaterialization,
}

impl GeneratedCSourceTemplateComponentV1 {
    pub const ALL: [Self; 9] = [
        Self::TypeRenderer,
        Self::LayoutAssertions,
        Self::ExternalDeclarations,
        Self::OutboundWrappers,
        Self::NativeGlobalAccessors,
        Self::CallbackTrampolines,
        Self::ForeignCallbackTrampolines,
        Self::UnitObjectPartition,
        Self::AtomBoundaryMaterialization,
    ];

    const fn tag(self) -> u32 {
        match self {
            Self::TypeRenderer => 1,
            Self::LayoutAssertions => 2,
            Self::ExternalDeclarations => 3,
            Self::OutboundWrappers => 4,
            Self::NativeGlobalAccessors => 5,
            Self::CallbackTrampolines => 6,
            Self::ForeignCallbackTrampolines => 7,
            Self::UnitObjectPartition => 8,
            Self::AtomBoundaryMaterialization => 9,
        }
    }
}

impl WireEncode for GeneratedCSourceTemplateComponentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedCSourceTemplateFingerprint([u8; 32]);

impl GeneratedCSourceTemplateFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for GeneratedCSourceTemplateFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for GeneratedCSourceTemplateFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

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
pub struct CBridgeToolchainContractV1 {
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    canonical_triple: &'static str,
    deployment: DarwinCBridgeDeploymentContractV1,
    compiler: AppleClangCompilerIdentityV1,
    canonical_flag_fingerprint: CanonicalCBridgeFlagFingerprint,
    source_template_fingerprint: GeneratedCSourceTemplateFingerprint,
    environment: CBridgeEnvironmentProjectionV1,
}

impl CBridgeToolchainContractV1 {
    fn darwin_aarch64_apple_clang(
        deployment: DarwinCBridgeDeploymentContractV1,
        compiler: AppleClangCompilerIdentityV1,
    ) -> Result<Self, HashError> {
        let target = LirTargetProfile::DARWIN_AARCH64;
        Ok(Self {
            target: target.wire_id(),
            target_fingerprint: target.fingerprint()?,
            canonical_triple: target.contract().canonical_triple(),
            deployment,
            compiler,
            canonical_flag_fingerprint: CanonicalCBridgeFlagContractV1::CURRENT.fingerprint()?,
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

    pub const fn deployment(&self) -> &DarwinCBridgeDeploymentContractV1 {
        &self.deployment
    }

    pub const fn compiler(&self) -> &AppleClangCompilerIdentityV1 {
        &self.compiler
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
        self.deployment.encode(encoder)?;
        encoder.field(5)?;
        self.compiler.encode(encoder)?;
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
    pub fn new_darwin_aarch64_apple_clang(
        deployment: DarwinCBridgeDeploymentContractV1,
        compiler: AppleClangCompilerIdentityV1,
    ) -> Result<Self, HashError> {
        let id = CBridgeToolchainProfileId::darwin_aarch64_apple_clang();
        let contract =
            CBridgeToolchainContractV1::darwin_aarch64_apple_clang(deployment, compiler)?;
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

    pub fn hash_stream_length(profile: &CBridgeToolchainProfileV1) -> Result<u64, HashError> {
        domain_separated_cbor_hash_stream_length(
            C_BRIDGE_TOOLCHAIN_PROFILE_DOMAIN,
            &CBridgeToolchainFingerprintInput {
                id: &profile.id,
                contract: &profile.contract,
            },
        )
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

#[cfg(test)]
mod tests;
