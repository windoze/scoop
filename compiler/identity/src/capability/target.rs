use std::sync::LazyLock;

use scoop_wire::{Encoder, WireEncode};

use super::{CapabilityId, CapabilityRefinementError, ObjectFormatId};

/// Closed executable targets; these identities include the libc environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetProfileId {
    DarwinAarch64,
    LinuxX86_64Gnu,
    LinuxX86_64Musl,
}

impl TargetProfileId {
    pub const ALL: [Self; 3] = [
        Self::DarwinAarch64,
        Self::LinuxX86_64Gnu,
        Self::LinuxX86_64Musl,
    ];

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "darwin-aarch64",
            Self::LinuxX86_64Gnu => "linux-x86-64-gnu",
            Self::LinuxX86_64Musl => "linux-x86-64-musl",
        }
    }

    pub const fn canonical_triple(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "aarch64-apple-darwin",
            Self::LinuxX86_64Gnu => "x86_64-unknown-linux-gnu",
            Self::LinuxX86_64Musl => "x86_64-unknown-linux-musl",
        }
    }

    pub const fn os(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "darwin",
            Self::LinuxX86_64Gnu | Self::LinuxX86_64Musl => "linux",
        }
    }

    pub const fn arch(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "aarch64",
            Self::LinuxX86_64Gnu | Self::LinuxX86_64Musl => "x86_64",
        }
    }

    pub const fn env(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "none",
            Self::LinuxX86_64Gnu => "gnu",
            Self::LinuxX86_64Musl => "musl",
        }
    }

    pub fn object_format(self) -> ObjectFormatId {
        match self {
            Self::DarwinAarch64 => ObjectFormatId::macho_relocatable(),
            Self::LinuxX86_64Gnu | Self::LinuxX86_64Musl => ObjectFormatId::elf_relocatable(),
        }
    }

    pub const fn external_symbol_prefix(self) -> &'static [u8] {
        match self {
            Self::DarwinAarch64 => b"_",
            Self::LinuxX86_64Gnu | Self::LinuxX86_64Musl => b"",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetProfileWireId {
    id: TargetProfileId,
}

impl TargetProfileWireId {
    pub const fn new(id: TargetProfileId) -> Self {
        Self { id }
    }

    pub fn darwin_aarch64() -> Self {
        Self::new(TargetProfileId::DarwinAarch64)
    }
    pub fn linux_x86_64_gnu() -> Self {
        Self::new(TargetProfileId::LinuxX86_64Gnu)
    }
    pub fn linux_x86_64_musl() -> Self {
        Self::new(TargetProfileId::LinuxX86_64Musl)
    }

    pub const fn id(&self) -> TargetProfileId {
        self.id
    }
    pub fn capability(&self) -> &CapabilityId {
        static DARWIN: LazyLock<CapabilityId> = LazyLock::new(|| {
            CapabilityId::known("org.scoop-lang.target-profile", "darwin-aarch64")
        });
        static GNU: LazyLock<CapabilityId> = LazyLock::new(|| {
            CapabilityId::known("org.scoop-lang.target-profile", "linux-x86-64-gnu")
        });
        static MUSL: LazyLock<CapabilityId> = LazyLock::new(|| {
            CapabilityId::known("org.scoop-lang.target-profile", "linux-x86-64-musl")
        });
        match self.id {
            TargetProfileId::DarwinAarch64 => &DARWIN,
            TargetProfileId::LinuxX86_64Gnu => &GNU,
            TargetProfileId::LinuxX86_64Musl => &MUSL,
        }
    }

    pub fn refine(capability: CapabilityId) -> Result<Self, CapabilityRefinementError> {
        let mut expected = Vec::new();
        for id in TargetProfileId::ALL {
            let target = Self::new(id);
            if target.capability() == &capability {
                return Ok(target);
            }
            expected.push(target.capability().clone());
        }
        Err(CapabilityRefinementError {
            expected,
            actual: capability,
        })
    }
}

impl WireEncode for TargetProfileWireId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.capability().encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodedCapabilityId;
    use scoop_wire::{decode_canonical, encode};

    #[test]
    fn target_wire_ids_are_closed_and_keep_their_platform() {
        for id in TargetProfileId::ALL {
            let profile = TargetProfileWireId::new(id);
            let decoded = decode_canonical::<DecodedCapabilityId>(&encode(&profile).unwrap())
                .unwrap()
                .validate()
                .unwrap();
            assert_eq!(TargetProfileWireId::refine(decoded).unwrap().id(), id);
        }
        for (name, version) in [("linux-aarch64-gnu", 1), ("linux-x86-64-gnu", 2)] {
            let value = CapabilityId::new("org.scoop-lang.target-profile", name, version).unwrap();
            assert!(TargetProfileWireId::refine(value).is_err());
        }
    }
}
