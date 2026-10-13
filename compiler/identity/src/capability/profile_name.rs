//! Inline spelling of an existing capability label and its numeric version.

use super::{CapabilityIdError, validate_label};

// Capability labels already have this canonical wire-format limit.
const MAX_LABEL_BYTES: usize = 63;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ProfileName {
    bytes: [u8; MAX_LABEL_BYTES],
    length: u8,
    version: u32,
}

impl ProfileName {
    pub(super) const fn new(name: &str, version: u32) -> Result<Self, CapabilityIdError> {
        let source = name.as_bytes();
        if let Err(error) = validate_label(name, MAX_LABEL_BYTES) {
            return Err(CapabilityIdError::Name(error));
        }
        if version == 0 {
            return Err(CapabilityIdError::ZeroMajorVersion);
        }
        let mut bytes = [0; MAX_LABEL_BYTES];
        let mut index = 0;
        while index < source.len() {
            let byte = source[index];
            bytes[index] = byte;
            index += 1;
        }
        Ok(Self {
            bytes,
            length: source.len() as u8,
            version,
        })
    }

    pub(super) fn name(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length as usize])
            .expect("profile labels contain only ASCII")
    }

    pub(super) const fn version(self) -> u32 {
        self.version
    }
}

impl std::fmt::Debug for ProfileName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}/{}", self.name(), self.version)
    }
}
