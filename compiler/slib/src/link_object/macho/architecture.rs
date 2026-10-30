//! Implemented Mach-O machine rules, independent of the container reader.

use object::{Architecture, macho};
use scoop_identity::TargetProfileId;
use scoop_lir::{NativeObjectFormat, TargetArchitecture};

use super::ObjectEnvelopeValidationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachOArchitecture {
    Aarch64,
}

impl MachOArchitecture {
    pub fn for_target(target: TargetProfileId) -> Result<Self, ObjectEnvelopeValidationError> {
        let profile = scoop_target_data::layout(target)
            .ok_or(ObjectEnvelopeValidationError::UnsupportedTarget)?;
        match (profile.native_object_format(), profile.architecture()) {
            (NativeObjectFormat::MachO64, TargetArchitecture::Aarch64) => Ok(Self::Aarch64),
            _ => Err(ObjectEnvelopeValidationError::UnsupportedTarget),
        }
    }

    pub const fn cpu_type(self) -> u32 {
        match self {
            Self::Aarch64 => macho::CPU_TYPE_ARM64,
        }
    }

    pub const fn cpu_subtype(self) -> u32 {
        match self {
            Self::Aarch64 => macho::CPU_SUBTYPE_ARM64_ALL,
        }
    }

    pub const fn object_architecture(self) -> Architecture {
        match self {
            Self::Aarch64 => Architecture::Aarch64,
        }
    }

    pub const fn text_stub_target(self) -> &'static str {
        match self {
            Self::Aarch64 => "arm64-macos",
        }
    }

    pub const fn system_text_stub_targets(self) -> &'static [&'static str] {
        match self {
            Self::Aarch64 => &["arm64-macos", "arm64e-macos"],
        }
    }
}
