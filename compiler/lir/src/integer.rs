use crate::{BackendScalarKind, LirType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntegerSignedness {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntegerWidth {
    W8,
    W16,
    W32,
    W64,
}

impl IntegerWidth {
    pub const fn bits(self) -> u32 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
            Self::W64 => 64,
        }
    }

    pub const fn bytes(self) -> u64 {
        (self.bits() / 8) as u64
    }

    pub const fn shift_mask(self) -> u64 {
        (self.bits() - 1) as u64
    }

    pub const fn scalar_type(self) -> LirType {
        match self {
            Self::W8 => LirType::I8,
            Self::W16 => LirType::I16,
            Self::W32 => LirType::I32,
            Self::W64 => LirType::I64,
        }
    }

    pub const fn backend_scalar_kind(self) -> BackendScalarKind {
        match self {
            Self::W8 => BackendScalarKind::I8,
            Self::W16 => BackendScalarKind::I16,
            Self::W32 => BackendScalarKind::I32,
            Self::W64 => BackendScalarKind::I64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntegerKind {
    signedness: IntegerSignedness,
    width: IntegerWidth,
}

impl IntegerKind {
    pub const SIGNED_8: Self = Self::new(IntegerSignedness::Signed, IntegerWidth::W8);
    pub const SIGNED_16: Self = Self::new(IntegerSignedness::Signed, IntegerWidth::W16);
    pub const SIGNED_32: Self = Self::new(IntegerSignedness::Signed, IntegerWidth::W32);
    pub const SIGNED_64: Self = Self::new(IntegerSignedness::Signed, IntegerWidth::W64);
    pub const UNSIGNED_8: Self = Self::new(IntegerSignedness::Unsigned, IntegerWidth::W8);
    pub const UNSIGNED_16: Self = Self::new(IntegerSignedness::Unsigned, IntegerWidth::W16);
    pub const UNSIGNED_32: Self = Self::new(IntegerSignedness::Unsigned, IntegerWidth::W32);
    pub const UNSIGNED_64: Self = Self::new(IntegerSignedness::Unsigned, IntegerWidth::W64);

    pub const ALL: [Self; 8] = [
        Self::SIGNED_8,
        Self::SIGNED_16,
        Self::SIGNED_32,
        Self::SIGNED_64,
        Self::UNSIGNED_8,
        Self::UNSIGNED_16,
        Self::UNSIGNED_32,
        Self::UNSIGNED_64,
    ];

    pub const fn new(signedness: IntegerSignedness, width: IntegerWidth) -> Self {
        Self { signedness, width }
    }

    pub const fn signedness(self) -> IntegerSignedness {
        self.signedness
    }

    pub const fn width(self) -> IntegerWidth {
        self.width
    }

    pub const fn scalar_type(self) -> LirType {
        self.width.scalar_type()
    }

    pub const fn canonical_name(self) -> &'static str {
        match (self.signedness, self.width) {
            (IntegerSignedness::Signed, IntegerWidth::W8) => "Int8",
            (IntegerSignedness::Signed, IntegerWidth::W16) => "Int16",
            (IntegerSignedness::Signed, IntegerWidth::W32) => "Int",
            (IntegerSignedness::Signed, IntegerWidth::W64) => "Long",
            (IntegerSignedness::Unsigned, IntegerWidth::W8) => "UInt8",
            (IntegerSignedness::Unsigned, IntegerWidth::W16) => "UInt16",
            (IntegerSignedness::Unsigned, IntegerWidth::W32) => "UInt",
            (IntegerSignedness::Unsigned, IntegerWidth::W64) => "ULong",
        }
    }
}

/// One source integer constant with a payload whose Rust width exactly matches
/// its Scoop width. Signed variants store the two's-complement raw bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LirIntegerConstant {
    Signed8(u8),
    Signed16(u16),
    Signed32(u32),
    Signed64(u64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
}

impl LirIntegerConstant {
    pub const fn kind(self) -> IntegerKind {
        match self {
            Self::Signed8(_) => IntegerKind::SIGNED_8,
            Self::Signed16(_) => IntegerKind::SIGNED_16,
            Self::Signed32(_) => IntegerKind::SIGNED_32,
            Self::Signed64(_) => IntegerKind::SIGNED_64,
            Self::Unsigned8(_) => IntegerKind::UNSIGNED_8,
            Self::Unsigned16(_) => IntegerKind::UNSIGNED_16,
            Self::Unsigned32(_) => IntegerKind::UNSIGNED_32,
            Self::Unsigned64(_) => IntegerKind::UNSIGNED_64,
        }
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::Signed8(bits) | Self::Unsigned8(bits) => bits as u64,
            Self::Signed16(bits) | Self::Unsigned16(bits) => bits as u64,
            Self::Signed32(bits) | Self::Unsigned32(bits) => bits as u64,
            Self::Signed64(bits) | Self::Unsigned64(bits) => bits,
        }
    }

    pub const fn scalar_type(self) -> LirType {
        self.kind().scalar_type()
    }

    pub fn dump(self) -> String {
        let digits = (self.kind().width().bits() / 4) as usize;
        format!(
            "integer<{}>(0x{:0digits$x})",
            self.kind().canonical_name(),
            self.raw_bits()
        )
    }
}
