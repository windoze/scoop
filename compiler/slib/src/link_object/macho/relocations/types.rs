use crate::link_object::ObjectRelocationTargetV1;
use std::{fmt, num::NonZeroU32};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachORelocationShapeV1 {
    Unsigned32 {
        target: ObjectRelocationTargetV1,
    },
    Subtractor32 {
        minuend: ObjectRelocationTargetV1,
        subtrahend: ObjectRelocationTargetV1,
    },
    X86_64PcRelative32 {
        kind: MachOX86_64PcRelativeKindV1,
        target: ObjectRelocationTargetV1,
    },
    Unsigned64 {
        target: ObjectRelocationTargetV1,
    },
    Subtractor64 {
        minuend: ObjectRelocationTargetV1,
        subtrahend: ObjectRelocationTargetV1,
    },
    Branch26 {
        target: ObjectRelocationTargetV1,
    },
    Page21 {
        target: ObjectRelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    PageOffset12 {
        target: ObjectRelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    GotLoadPage21 {
        target: ObjectRelocationTargetV1,
    },
    GotLoadPageOffset12 {
        target: ObjectRelocationTargetV1,
    },
    PointerToGot32 {
        target: ObjectRelocationTargetV1,
    },
    TlvpLoadPage21 {
        target: ObjectRelocationTargetV1,
    },
    TlvpLoadPageOffset12 {
        target: ObjectRelocationTargetV1,
    },
}

impl MachORelocationShapeV1 {
    pub const fn width_bytes(self) -> u8 {
        match self {
            Self::Unsigned64 { .. } | Self::Subtractor64 { .. } => 8,
            Self::Unsigned32 { .. }
            | Self::Subtractor32 { .. }
            | Self::X86_64PcRelative32 { .. }
            | Self::Branch26 { .. }
            | Self::Page21 { .. }
            | Self::PageOffset12 { .. }
            | Self::GotLoadPage21 { .. }
            | Self::GotLoadPageOffset12 { .. }
            | Self::PointerToGot32 { .. }
            | Self::TlvpLoadPage21 { .. }
            | Self::TlvpLoadPageOffset12 { .. } => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MachOX86_64PcRelativeKindV1 {
    Signed = object::macho::X86_64_RELOC_SIGNED,
    Branch = object::macho::X86_64_RELOC_BRANCH,
    GotLoad = object::macho::X86_64_RELOC_GOT_LOAD,
    Got = object::macho::X86_64_RELOC_GOT,
    Signed1 = object::macho::X86_64_RELOC_SIGNED_1,
    Signed2 = object::macho::X86_64_RELOC_SIGNED_2,
    Signed4 = object::macho::X86_64_RELOC_SIGNED_4,
    Tlv = object::macho::X86_64_RELOC_TLV,
}

impl MachOX86_64PcRelativeKindV1 {
    /// Bytes following the relocated displacement within the instruction.
    pub const fn trailing_bytes(self) -> u8 {
        match self {
            Self::Signed1 => 1,
            Self::Signed2 => 2,
            Self::Signed4 => 4,
            _ => 0,
        }
    }

    /// A non-external Mach-O displacement already contains the target address.
    pub(crate) fn section_target_address(self, field: u64, site: u64) -> Option<u64> {
        u64::try_from(
            i128::from(site) + 4 + i128::from(self.trailing_bytes()) + i128::from(field as i32),
        )
        .ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedMachORelocationV1 {
    pub(super) containing_section_ordinal: NonZeroU32,
    pub(super) offset: u32,
    pub(super) encoded_value: u64,
    pub(super) shape: MachORelocationShapeV1,
}

impl ObservedMachORelocationV1 {
    pub const fn containing_section_ordinal(&self) -> NonZeroU32 {
        self.containing_section_ordinal
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }

    pub const fn shape(&self) -> MachORelocationShapeV1 {
        self.shape
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachORelocationInventoryValidationError {
    TooManySections,
    RelocationTableOutOfBounds,
    ScatteredRelocation,
    UnsupportedRelocationKind {
        actual: u8,
    },
    InvalidRelocationFields {
        kind: u8,
        pcrel: bool,
        length: u8,
        external: bool,
    },
    MissingRelocationPair {
        first: u8,
    },
    InvalidRelocationPair {
        first: u8,
        second: u8,
    },
    SymbolTargetOutOfBounds {
        index: u32,
    },
    SectionTargetOutOfBounds {
        ordinal: u32,
    },
    RelocationSiteOutOfBounds,
    DuplicateRelocationOffset,
}

impl fmt::Display for MachORelocationInventoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Mach-O object relocation inventory: {self:?}"
        )
    }
}

impl std::error::Error for MachORelocationInventoryValidationError {}
