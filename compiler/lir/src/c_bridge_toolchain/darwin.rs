use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use super::{MAXIMUM_COMPILER_BUILD_LENGTH, encode_array, encode_unsigned_field};

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
