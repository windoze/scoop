use std::fmt;

use scoop_wire::{Encoder, WireEncodeV1};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId {
    namespace: String,
    name: String,
    major_version: u32,
}

impl CapabilityId {
    pub fn new(namespace: &str, name: &str, major_version: u32) -> Result<Self, CapabilityIdError> {
        validate_namespace(namespace)?;
        validate_label(name, 63).map_err(CapabilityIdError::Name)?;
        if major_version == 0 {
            return Err(CapabilityIdError::ZeroMajorVersion);
        }
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
}

impl WireEncodeV1 for CapabilityId {
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
        }

        impl WireEncodeV1 for $name {
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
capability_refinement!(
    BackendProfileWireId,
    llvm_22_1,
    "org.scoop-lang.backend-profile",
    "llvm-22-1"
);
capability_refinement!(
    ObjectFormatId,
    macho_relocatable,
    "org.scoop-lang.object-format",
    "mach-o-relocatable"
);
capability_refinement!(
    ArtifactCapabilityProfileId,
    identity_foundation,
    "org.scoop-lang.slib-profile",
    "identity-foundation"
);

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
    use scoop_wire::encode;

    use super::{CapabilityId, TargetProfileWireId};

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

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
