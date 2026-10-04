use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId {
    namespace: String,
    name: String,
    major_version: u32,
}

impl CapabilityId {
    pub fn new(namespace: &str, name: &str, major_version: u32) -> Result<Self, CapabilityIdError> {
        validate_capability(namespace, name, major_version)?;
        Ok(Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            major_version,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn major_version(&self) -> u32 {
        self.major_version
    }

    fn known(namespace: &'static str, name: &'static str) -> Self {
        Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            major_version: 1,
        }
    }

    fn from_owned(
        namespace: String,
        name: String,
        major_version: u32,
    ) -> Result<Self, CapabilityIdError> {
        validate_capability(&namespace, &name, major_version)?;
        Ok(Self {
            namespace,
            name,
            major_version,
        })
    }
}

fn validate_capability(
    namespace: &str,
    name: &str,
    major_version: u32,
) -> Result<(), CapabilityIdError> {
    validate_namespace(namespace)?;
    validate_label(name, 63).map_err(CapabilityIdError::Name)?;
    if major_version == 0 {
        return Err(CapabilityIdError::ZeroMajorVersion);
    }
    Ok(())
}

impl WireEncode for CapabilityId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.text(&self.namespace)?;
        encoder.field(2)?;
        encoder.text(&self.name)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.major_version))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCapabilityId {
    namespace: String,
    name: String,
    major_version: u32,
}

impl DecodedCapabilityId {
    pub fn validate(self) -> Result<CapabilityId, CapabilityIdError> {
        CapabilityId::from_owned(self.namespace, self.name, self.major_version)
    }
}

impl WireEncode for DecodedCapabilityId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.text(&self.namespace)?;
        encoder.field(2)?;
        encoder.text(&self.name)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.major_version))
    }
}

impl WireDecode for DecodedCapabilityId {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let namespace = decoder.field(1, Decoder::owned_text)?;
        let name = decoder.field(2, Decoder::owned_text)?;
        let major_version = decoder.field(3, Decoder::u32)?;
        Ok(Self {
            namespace,
            name,
            major_version,
        })
    }
}

macro_rules! capability_refinement {
    ($name:ident, $constructor:ident, $namespace:literal, $value:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(CapabilityId);

        impl $name {
            pub fn $constructor() -> Self {
                Self(CapabilityId::known($namespace, $value))
            }

            pub fn capability(&self) -> &CapabilityId {
                &self.0
            }

            pub fn refine(capability: CapabilityId) -> Result<Self, CapabilityRefinementError> {
                let expected = CapabilityId::known($namespace, $value);
                if capability == expected {
                    Ok(Self(capability))
                } else {
                    Err(CapabilityRefinementError {
                        expected,
                        actual: capability,
                    })
                }
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                self.0.encode(encoder)
            }
        }
    };
}

capability_refinement!(
    TargetProfileWireId,
    darwin_aarch64,
    "org.scoop-lang.target-profile",
    "darwin-aarch64"
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRefinementError {
    expected: CapabilityId,
    actual: CapabilityId,
}

impl CapabilityRefinementError {
    pub const fn expected(&self) -> &CapabilityId {
        &self.expected
    }

    pub const fn actual(&self) -> &CapabilityId {
        &self.actual
    }
}

impl fmt::Display for CapabilityRefinementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "expected capability {}/{}/{}, found {}/{}/{}",
            self.expected.namespace(),
            self.expected.name(),
            self.expected.major_version(),
            self.actual.namespace(),
            self.actual.name(),
            self.actual.major_version(),
        )
    }
}

impl std::error::Error for CapabilityRefinementError {}
capability_refinement!(
    BackendProfileWireId,
    llvm_22_1,
    "org.scoop-lang.backend-profile",
    "llvm-22-1"
);
capability_refinement!(
    CBridgeToolchainProfileId,
    darwin_aarch64_apple_clang,
    "org.scoop-lang.c-bridge-toolchain-profile",
    "darwin-aarch64-apple-clang"
);
capability_refinement!(
    ObjectFormatId,
    macho_relocatable,
    "org.scoop-lang.object-format",
    "mach-o-relocatable"
);
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCapabilityProfileId(CapabilityId);

impl ArtifactCapabilityProfileId {
    pub fn single_cone_strong() -> Self {
        Self(CapabilityId {
            namespace: "org.scoop-lang.slib-profile".to_owned(),
            name: "single-cone-strong".to_owned(),
            major_version: 5,
        })
    }

    pub fn cross_cone_semantics_strong() -> Self {
        Self(CapabilityId {
            namespace: "org.scoop-lang.slib-profile".to_owned(),
            name: "cross-cone-semantics-strong".to_owned(),
            major_version: 5,
        })
    }

    pub fn cross_cone_generic() -> Self {
        Self(CapabilityId {
            namespace: "org.scoop-lang.slib-profile".to_owned(),
            name: "cross-cone-generic".to_owned(),
            major_version: 4,
        })
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.0
    }

    pub fn refine(
        capability: CapabilityId,
    ) -> Result<Self, ArtifactCapabilityProfileRefinementError> {
        let single_cone_strong = Self::single_cone_strong();
        let cross_cone_semantics_strong = Self::cross_cone_semantics_strong();
        let cross_cone_generic = Self::cross_cone_generic();
        if capability == single_cone_strong.0
            || capability == cross_cone_semantics_strong.0
            || capability == cross_cone_generic.0
        {
            Ok(Self(capability))
        } else {
            Err(ArtifactCapabilityProfileRefinementError { actual: capability })
        }
    }
}

impl WireEncode for ArtifactCapabilityProfileId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCapabilityProfileRefinementError {
    actual: CapabilityId,
}

impl ArtifactCapabilityProfileRefinementError {
    pub const fn actual(&self) -> &CapabilityId {
        &self.actual
    }
}

impl fmt::Display for ArtifactCapabilityProfileRefinementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unregistered artifact capability profile {}/{}/{}",
            self.actual.namespace(),
            self.actual.name(),
            self.actual.major_version(),
        )
    }
}

impl std::error::Error for ArtifactCapabilityProfileRefinementError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityIdError {
    NamespaceTooLong,
    EmptyNamespaceSegment,
    NamespaceSegment(CapabilityLabelError),
    Name(CapabilityLabelError),
    ZeroMajorVersion,
}

impl fmt::Display for CapabilityIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamespaceTooLong => {
                formatter.write_str("capability namespace exceeds 255 ASCII bytes")
            }
            Self::EmptyNamespaceSegment => {
                formatter.write_str("capability namespace contains an empty segment")
            }
            Self::NamespaceSegment(error) => {
                write!(formatter, "invalid capability namespace segment: {error}")
            }
            Self::Name(error) => write!(formatter, "invalid capability name: {error}"),
            Self::ZeroMajorVersion => {
                formatter.write_str("capability major version must be nonzero")
            }
        }
    }
}

impl std::error::Error for CapabilityIdError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityLabelError {
    Empty,
    TooLong,
    InvalidStart,
    InvalidContinuation,
}

impl fmt::Display for CapabilityLabelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "label must not be empty",
            Self::TooLong => "label exceeds its byte limit",
            Self::InvalidStart => "label must start with a lowercase ASCII letter",
            Self::InvalidContinuation => {
                "label may contain only lowercase ASCII letters, digits, and '-'"
            }
        })
    }
}

impl std::error::Error for CapabilityLabelError {}

fn validate_namespace(value: &str) -> Result<(), CapabilityIdError> {
    if value.len() > 255 {
        return Err(CapabilityIdError::NamespaceTooLong);
    }
    for segment in value.split('.') {
        if segment.is_empty() {
            return Err(CapabilityIdError::EmptyNamespaceSegment);
        }
        validate_label(segment, 63).map_err(CapabilityIdError::NamespaceSegment)?;
    }
    Ok(())
}

fn validate_label(value: &str, maximum_length: usize) -> Result<(), CapabilityLabelError> {
    if value.is_empty() {
        return Err(CapabilityLabelError::Empty);
    }
    if value.len() > maximum_length {
        return Err(CapabilityLabelError::TooLong);
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
        return Err(CapabilityLabelError::InvalidStart);
    }
    if !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-') {
        return Err(CapabilityLabelError::InvalidContinuation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::{decode_canonical, encode};

    use super::{
        ArtifactCapabilityProfileId, CBridgeToolchainProfileId, CapabilityId, DecodedCapabilityId,
        TargetProfileWireId,
    };

    #[test]
    fn capability_grammar_and_numeric_major_are_canonical() {
        assert!(CapabilityId::new("org.scoop-lang.hir", "identity-foundation", 1).is_ok());
        for (namespace, name, major) in [
            ("", "name", 1),
            ("org..hir", "name", 1),
            ("Org.hir", "name", 1),
            ("org.hir", "Name", 1),
            ("org.hir", "name", 0),
        ] {
            assert!(CapabilityId::new(namespace, name, major).is_err());
        }
    }

    #[test]
    fn target_profile_refinement_has_fixed_wire() {
        assert_eq!(
            hex(&encode(&TargetProfileWireId::darwin_aarch64()).unwrap()),
            "a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d616172636836340301"
        );
    }

    #[test]
    fn c_bridge_toolchain_profile_refinement_has_fixed_wire() {
        assert_eq!(
            hex(&encode(&CBridgeToolchainProfileId::darwin_aarch64_apple_clang()).unwrap()),
            "a30178296f72672e73636f6f702d6c616e672e632d6272696467652d746f6f6c636861696e2d70726f66696c6502781a64617277696e2d616172636836342d6170706c652d636c616e670301"
        );
    }

    #[test]
    fn artifact_profile_refinement_accepts_all_registered_profiles() {
        for profile in [
            ArtifactCapabilityProfileId::single_cone_strong(),
            ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
            ArtifactCapabilityProfileId::cross_cone_generic(),
        ] {
            assert_eq!(
                ArtifactCapabilityProfileId::refine(profile.capability().clone()),
                Ok(profile)
            );
        }
        assert!(
            ArtifactCapabilityProfileId::refine(
                CapabilityId::new("org.scoop-lang.slib-profile", "unknown", 1).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn retired_artifact_profiles_require_rebuilding() {
        for (name, last_major) in [
            ("identity-foundation", 2),
            ("single-cone-strong", 2),
            ("cross-cone-semantics-strong", 2),
            ("cross-cone-layout-strong", 3),
            ("cross-cone-generic", 1),
        ] {
            for major in 1..=last_major {
                let retired =
                    CapabilityId::new("org.scoop-lang.slib-profile", name, major).unwrap();
                assert!(ArtifactCapabilityProfileId::refine(retired).is_err());
            }
        }
    }

    #[test]
    fn capability_decode_validates_grammar_before_refinement() {
        let target = TargetProfileWireId::darwin_aarch64();
        let decoded =
            decode_canonical::<DecodedCapabilityId>(&encode(target.capability()).unwrap()).unwrap();
        assert_eq!(
            TargetProfileWireId::refine(decoded.validate().unwrap()),
            Ok(target)
        );

        let malformed = b"\xa3\x01\x63Org\x02\x64name\x03\x01";
        assert!(
            decode_canonical::<DecodedCapabilityId>(malformed)
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(
            TargetProfileWireId::refine(
                CapabilityId::new("org.scoop-lang.target-profile", "other", 1).unwrap()
            )
            .is_err()
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
