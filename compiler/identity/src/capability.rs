//! Versioned capability ids and the two typed wrappers used on the wire
//! (`TargetProfileWireId`, `ObjectFormatId`), with the built-in registry
//! frozen by DESIGN section 4.1.

use core::fmt;

use crate::cbor::CborWriter;

/// Error constructing a [`CapabilityId`] from non-canonical text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    NamespaceTooLong(usize),
    NameTooLong(usize),
    EmptyNamespace,
    EmptyName,
    InvalidNamespaceByte(u8),
    InvalidNameByte(u8),
    EmptySegment,
    ZeroMajorVersion,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapabilityError::NamespaceTooLong(len) => {
                write!(f, "capability namespace is {len} bytes, limit is 255")
            }
            CapabilityError::NameTooLong(len) => {
                write!(f, "capability name is {len} bytes, limit is 63")
            }
            CapabilityError::EmptyNamespace => write!(f, "capability namespace must not be empty"),
            CapabilityError::EmptyName => write!(f, "capability name must not be empty"),
            CapabilityError::InvalidNamespaceByte(byte) => {
                write!(f, "capability namespace contains invalid byte 0x{byte:02x}")
            }
            CapabilityError::InvalidNameByte(byte) => {
                write!(f, "capability name contains invalid byte 0x{byte:02x}")
            }
            CapabilityError::EmptySegment => {
                write!(
                    f,
                    "capability namespace contains an empty '.'-separated segment"
                )
            }
            CapabilityError::ZeroMajorVersion => {
                write!(f, "capability major version must be at least 1")
            }
        }
    }
}

impl std::error::Error for CapabilityError {}

/// A versioned producer/verifier capability: `{namespace, name,
/// major_version}` (DESIGN section 4.1). Namespaces are dot-joined
/// lowercase segments; the wire form is `{1: namespace, 2: name,
/// 3: major}`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CapabilityId {
    namespace: String,
    name: String,
    major: u32,
}

impl CapabilityId {
    pub fn new(namespace: &str, name: &str, major: u32) -> Result<Self, CapabilityError> {
        if namespace.is_empty() {
            return Err(CapabilityError::EmptyNamespace);
        }
        if namespace.len() > 255 {
            return Err(CapabilityError::NamespaceTooLong(namespace.len()));
        }
        for segment in namespace.split('.') {
            validate_label(segment, true)?;
        }
        validate_label(name, false)?;
        if major == 0 {
            return Err(CapabilityError::ZeroMajorVersion);
        }
        Ok(CapabilityId {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            major,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn major_version(&self) -> u32 {
        self.major
    }

    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        writer.map(3);
        writer.field(1).text(&self.namespace);
        writer.field(2).text(&self.name);
        writer.field(3).unsigned(self.major as u64);
        writer.into_bytes()
    }

    fn display(&self) -> String {
        format!("{}/{} v{}", self.namespace, self.name, self.major)
    }
}

/// Validates one `[a-z][a-z0-9-]{0,62}` label (at most 63 bytes).
fn validate_label(label: &str, namespace_segment: bool) -> Result<(), CapabilityError> {
    if label.is_empty() {
        return Err(if namespace_segment {
            CapabilityError::EmptySegment
        } else {
            CapabilityError::EmptyName
        });
    }
    if label.len() > 63 {
        return Err(if namespace_segment {
            CapabilityError::NamespaceTooLong(label.len())
        } else {
            CapabilityError::NameTooLong(label.len())
        });
    }
    let mut bytes = label.bytes();
    let first = bytes.next().expect("non-empty");
    if !first.is_ascii_lowercase() {
        return Err(if namespace_segment {
            CapabilityError::InvalidNamespaceByte(first)
        } else {
            CapabilityError::InvalidNameByte(first)
        });
    }
    for byte in bytes {
        let ok = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !ok {
            return Err(if namespace_segment {
                CapabilityError::InvalidNamespaceByte(byte)
            } else {
                CapabilityError::InvalidNameByte(byte)
            });
        }
    }
    Ok(())
}

impl fmt::Debug for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapabilityId({:?})", self.display())
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

/// Typed wrapper: a target profile identity on the wire. Distinct from
/// [`ObjectFormatId`] and bare [`CapabilityId`] at the type level.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetProfileWireId(CapabilityId);

impl TargetProfileWireId {
    pub fn new(capability: CapabilityId) -> Self {
        TargetProfileWireId(capability)
    }

    pub fn as_capability(&self) -> &CapabilityId {
        &self.0
    }

    /// `org.scoop-lang.target-profile / darwin-aarch64 / 1`.
    pub fn darwin_aarch64_v1() -> Self {
        TargetProfileWireId(
            CapabilityId::new("org.scoop-lang.target-profile", "darwin-aarch64", 1)
                .expect("built-in target profile capability is canonical"),
        )
    }
}

impl fmt::Debug for TargetProfileWireId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TargetProfileWireId({:?})", self.0)
    }
}

/// Typed wrapper: a native object format identity on the wire.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectFormatId(CapabilityId);

impl ObjectFormatId {
    pub fn new(capability: CapabilityId) -> Self {
        ObjectFormatId(capability)
    }

    pub fn as_capability(&self) -> &CapabilityId {
        &self.0
    }

    /// `org.scoop-lang.object-format / mach-o-relocatable / 1`.
    pub fn mach_o_relocatable_v1() -> Self {
        ObjectFormatId(
            CapabilityId::new("org.scoop-lang.object-format", "mach-o-relocatable", 1)
                .expect("built-in object format capability is canonical"),
        )
    }
}

impl fmt::Debug for ObjectFormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectFormatWireId({:?})", self.0)
    }
}

/// The built-in Scoop LIR link-object verifier capability:
/// `org.scoop-lang.link-object / scoop-lir / 1`.
pub fn scoop_lir_link_object_v1() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.link-object", "scoop-lir", 1)
        .expect("built-in link object capability is canonical")
}

/// The built-in generated C bridge link-object verifier capability:
/// `org.scoop-lang.link-object / generated-c-bridge / 1`.
pub fn generated_c_bridge_link_object_v1() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.link-object", "generated-c-bridge", 1)
        .expect("built-in link object capability is canonical")
}
