use std::fmt;

/// Source-level signedness of one fixed-width integer representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerSignedness {
    Signed,
    Unsigned,
}

/// Source-level width of one fixed-width integer representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub const fn bytes(self) -> u32 {
        self.bits() / 8
    }

    pub const fn raw_mask(self) -> u64 {
        match self {
            Self::W8 => u8::MAX as u64,
            Self::W16 => u16::MAX as u64,
            Self::W32 => u32::MAX as u64,
            Self::W64 => u64::MAX,
        }
    }

    const fn signed_positive_max(self) -> u64 {
        match self {
            Self::W8 => i8::MAX as u64,
            Self::W16 => i16::MAX as u64,
            Self::W32 => i32::MAX as u64,
            Self::W64 => i64::MAX as u64,
        }
    }

    const fn signed_negative_magnitude_max(self) -> u64 {
        self.signed_positive_max() + 1
    }
}

/// Exact semantic representation of one canonical source integer type.
///
/// The nominal owner remains a separate typed entity. This value records the
/// representation fact that consumers need without deriving signedness or
/// width from a source name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub const COUNT: usize = Self::ALL.len();

    pub const fn new(signedness: IntegerSignedness, width: IntegerWidth) -> Self {
        Self { signedness, width }
    }

    pub const fn signedness(self) -> IntegerSignedness {
        self.signedness
    }

    pub const fn width(self) -> IntegerWidth {
        self.width
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::SIGNED_8 => "Int8",
            Self::SIGNED_16 => "Int16",
            Self::SIGNED_32 => "Int",
            Self::SIGNED_64 => "Long",
            Self::UNSIGNED_8 => "UInt8",
            Self::UNSIGNED_16 => "UInt16",
            Self::UNSIGNED_32 => "UInt",
            Self::UNSIGNED_64 => "ULong",
        }
    }

    pub const fn registry_key(self) -> &'static str {
        match self {
            Self::SIGNED_8 => "int8",
            Self::SIGNED_16 => "int16",
            Self::SIGNED_32 => "int",
            Self::SIGNED_64 => "long",
            Self::UNSIGNED_8 => "uint8",
            Self::UNSIGNED_16 => "uint16",
            Self::UNSIGNED_32 => "uint",
            Self::UNSIGNED_64 => "ulong",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::SIGNED_8 => "core_int8",
            Self::SIGNED_16 => "core_int16",
            Self::SIGNED_32 => "core_int",
            Self::SIGNED_64 => "core_long",
            Self::UNSIGNED_8 => "core_uint8",
            Self::UNSIGNED_16 => "core_uint16",
            Self::UNSIGNED_32 => "core_uint",
            Self::UNSIGNED_64 => "core_ulong",
        }
    }

    pub const fn fits_positive_magnitude(self, magnitude: u64) -> bool {
        match self.signedness {
            IntegerSignedness::Signed => magnitude <= self.width.signed_positive_max(),
            IntegerSignedness::Unsigned => magnitude <= self.width.raw_mask(),
        }
    }

    pub const fn fits_negative_magnitude(self, magnitude: u64) -> bool {
        matches!(self.signedness, IntegerSignedness::Signed)
            && magnitude <= self.width.signed_negative_magnitude_max()
    }

    pub const fn fits_magnitude(self, magnitude: u64, negative: bool) -> bool {
        if negative {
            self.fits_negative_magnitude(magnitude)
        } else {
            self.fits_positive_magnitude(magnitude)
        }
    }

    pub const fn fits_i128(self, value: i128) -> bool {
        if value < 0 {
            let magnitude = value.unsigned_abs();
            magnitude <= u64::MAX as u128 && self.fits_negative_magnitude(magnitude as u64)
        } else {
            value <= u64::MAX as i128 && self.fits_positive_magnitude(value as u64)
        }
    }

    pub const fn fits_u128(self, value: u128) -> bool {
        value <= u64::MAX as u128 && self.fits_positive_magnitude(value as u64)
    }

    const fn index(self) -> usize {
        match self {
            Self::SIGNED_8 => 0,
            Self::SIGNED_16 => 1,
            Self::SIGNED_32 => 2,
            Self::SIGNED_64 => 3,
            Self::UNSIGNED_8 => 4,
            Self::UNSIGNED_16 => 5,
            Self::UNSIGNED_32 => 6,
            Self::UNSIGNED_64 => 7,
        }
    }
}

/// A width-exact HIR integer constant. Signed payloads contain the raw
/// two's-complement bits in the corresponding unsigned host type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirIntegerConstant {
    Signed8(u8),
    Signed16(u16),
    Signed32(u32),
    Signed64(u64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
}

impl HirIntegerConstant {
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
            Self::Signed8(value) | Self::Unsigned8(value) => value as u64,
            Self::Signed16(value) | Self::Unsigned16(value) => value as u64,
            Self::Signed32(value) | Self::Unsigned32(value) => value as u64,
            Self::Signed64(value) | Self::Unsigned64(value) => value,
        }
    }

    pub const fn from_magnitude(kind: IntegerKind, magnitude: u64, negative: bool) -> Option<Self> {
        if !kind.fits_magnitude(magnitude, negative) {
            return None;
        }

        let raw = if negative {
            0u64.wrapping_sub(magnitude) & kind.width.raw_mask()
        } else {
            magnitude
        };
        Some(match kind {
            IntegerKind::SIGNED_8 => Self::Signed8(raw as u8),
            IntegerKind::SIGNED_16 => Self::Signed16(raw as u16),
            IntegerKind::SIGNED_32 => Self::Signed32(raw as u32),
            IntegerKind::SIGNED_64 => Self::Signed64(raw),
            IntegerKind::UNSIGNED_8 => Self::Unsigned8(raw as u8),
            IntegerKind::UNSIGNED_16 => Self::Unsigned16(raw as u16),
            IntegerKind::UNSIGNED_32 => Self::Unsigned32(raw as u32),
            IntegerKind::UNSIGNED_64 => Self::Unsigned64(raw),
        })
    }
}

impl fmt::Display for HirIntegerConstant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Signed8(raw) => write!(formatter, "{}", raw as i8),
            Self::Signed16(raw) => write!(formatter, "{}", raw as i16),
            Self::Signed32(raw) => write!(formatter, "{}", raw as i32),
            Self::Signed64(raw) => write!(formatter, "{}", raw as i64),
            Self::Unsigned8(raw) => write!(formatter, "{raw}u"),
            Self::Unsigned16(raw) => write!(formatter, "{raw}u"),
            Self::Unsigned32(raw) => write!(formatter, "{raw}u"),
            Self::Unsigned64(raw) => write!(formatter, "{raw}uL"),
        }
    }
}

/// The eight canonical integer owners for one HIR identity family.
///
/// Array order follows [`IntegerKind::ALL`]. Construction rejects a repeated
/// owner so reverse lookup remains unambiguous, while `owner` is total for
/// every possible `IntegerKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerTypeCore<Owner> {
    owners: [Owner; IntegerKind::COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateIntegerOwner;

impl<Owner: Copy + Eq> IntegerTypeCore<Owner> {
    pub fn new(owners: [Owner; IntegerKind::COUNT]) -> Result<Self, DuplicateIntegerOwner> {
        for (index, owner) in owners.iter().enumerate() {
            if owners[..index].contains(owner) {
                return Err(DuplicateIntegerOwner);
            }
        }
        Ok(Self { owners })
    }

    pub const fn owner(&self, kind: IntegerKind) -> Owner {
        self.owners[kind.index()]
    }

    pub fn kind_for_owner(&self, owner: Owner) -> Option<IntegerKind> {
        IntegerKind::ALL
            .into_iter()
            .find(|kind| self.owner(*kind) == owner)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (IntegerKind, Owner)> + '_ {
        IntegerKind::ALL
            .into_iter()
            .map(|kind| (kind, self.owner(kind)))
    }
}

/// Integer operations whose declarations and call targets must be `@NoGC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoGcIntegerOperation {
    UnaryPlus,
    UnaryMinus,
    Inc,
    Dec,
    Add,
    Sub,
    Mul,
    CompareTo,
    Equals,
    And,
    Or,
    Xor,
    Inv,
    Shl,
    Shr,
    Ushr,
}

impl NoGcIntegerOperation {
    pub const ALL: [Self; 16] = [
        Self::UnaryPlus,
        Self::UnaryMinus,
        Self::Inc,
        Self::Dec,
        Self::Add,
        Self::Sub,
        Self::Mul,
        Self::CompareTo,
        Self::Equals,
        Self::And,
        Self::Or,
        Self::Xor,
        Self::Inv,
        Self::Shl,
        Self::Shr,
        Self::Ushr,
    ];

    pub const fn registry_key(self) -> &'static str {
        match self {
            Self::UnaryPlus => "unary_plus",
            Self::UnaryMinus => "unary_minus",
            Self::Inc => "inc",
            Self::Dec => "dec",
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::CompareTo => "compare_to",
            Self::Equals => "equals",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Inv => "inv",
            Self::Shl => "shl",
            Self::Shr => "shr",
            Self::Ushr => "ushr",
        }
    }

    pub const fn arity(self) -> IntegerOperationArity {
        match self {
            Self::UnaryPlus | Self::UnaryMinus | Self::Inc | Self::Dec | Self::Inv => {
                IntegerOperationArity::Unary
            }
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::CompareTo
            | Self::Equals
            | Self::And
            | Self::Or
            | Self::Xor
            | Self::Shl
            | Self::Shr
            | Self::Ushr => IntegerOperationArity::Binary,
        }
    }

    pub const fn supports(self, kind: IntegerKind) -> bool {
        !matches!(self, Self::Ushr) || matches!(kind.signedness(), IntegerSignedness::Signed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerDivRem {
    Div,
    Rem,
}

impl IntegerDivRem {
    pub const ALL: [Self; 2] = [Self::Div, Self::Rem];

    pub const fn registry_key(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::Rem => "rem",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerOperationArity {
    Unary,
    Binary,
}

/// A normalized integer operation. The closed variants encode its effect;
/// div/rem cannot be represented as a no-GC operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerOperation {
    NoGc {
        kind: IntegerKind,
        operation: NoGcIntegerOperation,
    },
    Managed {
        kind: IntegerKind,
        operation: IntegerDivRem,
    },
}

impl IntegerOperation {
    pub const fn kind(&self) -> IntegerKind {
        match self {
            Self::NoGc { kind, .. } | Self::Managed { kind, .. } => *kind,
        }
    }

    pub const fn arity(&self) -> IntegerOperationArity {
        match self {
            Self::NoGc { operation, .. } => operation.arity(),
            Self::Managed { .. } => IntegerOperationArity::Binary,
        }
    }
}

/// A normalized, always-no-GC integer conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerConversion {
    pub source: IntegerKind,
    pub target_kind: IntegerKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_total_canonical_metadata() {
        let expected = [
            (IntegerKind::SIGNED_8, "Int8", "int8", "core_int8"),
            (IntegerKind::SIGNED_16, "Int16", "int16", "core_int16"),
            (IntegerKind::SIGNED_32, "Int", "int", "core_int"),
            (IntegerKind::SIGNED_64, "Long", "long", "core_long"),
            (IntegerKind::UNSIGNED_8, "UInt8", "uint8", "core_uint8"),
            (IntegerKind::UNSIGNED_16, "UInt16", "uint16", "core_uint16"),
            (IntegerKind::UNSIGNED_32, "UInt", "uint", "core_uint"),
            (IntegerKind::UNSIGNED_64, "ULong", "ulong", "core_ulong"),
        ];

        assert_eq!(IntegerKind::ALL.len(), expected.len());
        for (kind, name, key, intrinsic) in expected {
            assert_eq!(kind.canonical_name(), name);
            assert_eq!(kind.registry_key(), key);
            assert_eq!(kind.intrinsic_name(), intrinsic);
        }
    }

    #[test]
    fn literal_fit_includes_signed_min_and_unsigned_max() {
        assert!(IntegerKind::SIGNED_8.fits_positive_magnitude(127));
        assert!(!IntegerKind::SIGNED_8.fits_positive_magnitude(128));
        assert!(IntegerKind::SIGNED_8.fits_negative_magnitude(128));
        assert!(!IntegerKind::SIGNED_8.fits_negative_magnitude(129));
        assert!(!IntegerKind::UNSIGNED_8.fits_negative_magnitude(1));
        assert!(IntegerKind::UNSIGNED_64.fits_u128(u64::MAX as u128));
        assert!(!IntegerKind::SIGNED_64.fits_u128(u64::MAX as u128));
        assert!(!IntegerKind::UNSIGNED_64.fits_i128(i128::MAX));
    }

    #[test]
    fn constants_preserve_width_exact_raw_bits() {
        let signed_min = HirIntegerConstant::from_magnitude(IntegerKind::SIGNED_8, 128, true)
            .expect("signed minimum fits");
        let unsigned_max =
            HirIntegerConstant::from_magnitude(IntegerKind::UNSIGNED_64, u64::MAX, false)
                .expect("unsigned maximum fits");

        assert_eq!(signed_min, HirIntegerConstant::Signed8(0x80));
        assert_eq!(signed_min.raw_bits(), 0x80);
        assert_eq!(signed_min.kind(), IntegerKind::SIGNED_8);
        assert_eq!(signed_min.to_string(), "-128");
        assert_eq!(unsigned_max, HirIntegerConstant::Unsigned64(u64::MAX));
        assert_eq!(unsigned_max.raw_bits(), u64::MAX);
        assert_eq!(unsigned_max.to_string(), "18446744073709551615uL");
    }

    #[test]
    fn integer_core_is_total_and_rejects_duplicate_owners() {
        let core = IntegerTypeCore::new([0, 1, 2, 3, 4, 5, 6, 7]).expect("unique owners");
        for (index, kind) in IntegerKind::ALL.into_iter().enumerate() {
            assert_eq!(core.owner(kind), index);
            assert_eq!(core.kind_for_owner(index), Some(kind));
        }
        assert_eq!(core.kind_for_owner(8), None);
        assert_eq!(
            IntegerTypeCore::new([0, 1, 2, 3, 4, 5, 6, 6]),
            Err(DuplicateIntegerOwner)
        );
    }

    #[test]
    fn integer_operation_effect_and_domain_are_closed() {
        assert_eq!(
            NoGcIntegerOperation::UnaryMinus.arity(),
            IntegerOperationArity::Unary
        );
        assert_eq!(
            NoGcIntegerOperation::Shl.arity(),
            IntegerOperationArity::Binary
        );
        assert!(NoGcIntegerOperation::Ushr.supports(IntegerKind::SIGNED_32));
        assert!(!NoGcIntegerOperation::Ushr.supports(IntegerKind::UNSIGNED_32));
    }
}
