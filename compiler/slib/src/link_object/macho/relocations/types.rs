use std::{fmt, num::NonZeroU32};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationTargetV1 {
    SymbolTableIndex(u32),
    SectionOrdinal(NonZeroU32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationShapeV1 {
    Unsigned64 {
        target: DarwinArm64RelocationTargetV1,
    },
    Subtractor64 {
        minuend: DarwinArm64RelocationTargetV1,
        subtrahend: DarwinArm64RelocationTargetV1,
    },
    Branch26 {
        target: DarwinArm64RelocationTargetV1,
    },
    Page21 {
        target: DarwinArm64RelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    PageOffset12 {
        target: DarwinArm64RelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    GotLoadPage21 {
        target: DarwinArm64RelocationTargetV1,
    },
    GotLoadPageOffset12 {
        target: DarwinArm64RelocationTargetV1,
    },
    PointerToGot32 {
        target: DarwinArm64RelocationTargetV1,
    },
    TlvpLoadPage21 {
        target: DarwinArm64RelocationTargetV1,
    },
    TlvpLoadPageOffset12 {
        target: DarwinArm64RelocationTargetV1,
    },
}

impl DarwinArm64RelocationShapeV1 {
    pub const fn width_bytes(self) -> u8 {
        match self {
            Self::Unsigned64 { .. } | Self::Subtractor64 { .. } => 8,
            Self::Branch26 { .. }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedMachORelocationV1 {
    pub(super) containing_section_ordinal: NonZeroU32,
    pub(super) offset: u32,
    pub(super) encoded_value: u64,
    pub(super) shape: DarwinArm64RelocationShapeV1,
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

    pub const fn shape(&self) -> DarwinArm64RelocationShapeV1 {
        self.shape
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationInventoryValidationError {
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

impl fmt::Display for DarwinArm64RelocationInventoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin/AArch64 object relocation inventory: {self:?}"
        )
    }
}

impl std::error::Error for DarwinArm64RelocationInventoryValidationError {}
