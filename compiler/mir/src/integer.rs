/// Signedness of one fixed-width Scoop source integer.
///
/// This is a source-language semantic property. It must not be reused for
/// compiler-owned scalar domains such as enum tags or statepoint ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntegerSignedness {
    Signed,
    Unsigned,
}

impl IntegerSignedness {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Signed => "signed",
            Self::Unsigned => "unsigned",
        }
    }
}

/// Storage width of one fixed-width Scoop source integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntegerWidth {
    W8,
    W16,
    W32,
    W64,
}

impl IntegerWidth {
    pub const fn bits(self) -> u8 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
            Self::W64 => 64,
        }
    }

    pub const fn bytes(self) -> u8 {
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
}

/// Exact semantic kind of one Scoop source integer.
///
/// All eight combinations are valid. Keeping the two axes in one value makes
/// signedness available even after the physical LLVM integer type has erased
/// it.
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

/// A width-matched source integer constant.
///
/// Signed variants deliberately store raw bits in the corresponding unsigned
/// Rust type. This admits every two's-complement bit pattern, including each
/// signed minimum, without a parallel kind field that could disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirIntegerConstant {
    Signed8(u8),
    Signed16(u16),
    Signed32(u32),
    Signed64(u64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
}

impl MirIntegerConstant {
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

    pub const fn signedness(self) -> IntegerSignedness {
        self.kind().signedness()
    }

    pub const fn width(self) -> IntegerWidth {
        self.kind().width()
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::Signed8(bits) | Self::Unsigned8(bits) => bits as u64,
            Self::Signed16(bits) | Self::Unsigned16(bits) => bits as u64,
            Self::Signed32(bits) | Self::Unsigned32(bits) => bits as u64,
            Self::Signed64(bits) | Self::Unsigned64(bits) => bits,
        }
    }

    /// Mathematical value obtained by interpreting the exact-width bits with
    /// this constant's signedness. `i128` covers all eight domains.
    pub const fn mathematical_value(self) -> i128 {
        match self {
            Self::Signed8(bits) => bits as i8 as i128,
            Self::Signed16(bits) => bits as i16 as i128,
            Self::Signed32(bits) => bits as i32 as i128,
            Self::Signed64(bits) => bits as i64 as i128,
            Self::Unsigned8(bits) => bits as i128,
            Self::Unsigned16(bits) => bits as i128,
            Self::Unsigned32(bits) => bits as i128,
            Self::Unsigned64(bits) => bits as i128,
        }
    }

    /// Construct from exact-width raw bits, rejecting bits outside the stated
    /// width instead of silently truncating them.
    pub const fn from_raw_bits(kind: IntegerKind, raw_bits: u64) -> Option<Self> {
        if raw_bits & !kind.width().raw_mask() != 0 {
            return None;
        }
        Some(match (kind.signedness(), kind.width()) {
            (IntegerSignedness::Signed, IntegerWidth::W8) => Self::Signed8(raw_bits as u8),
            (IntegerSignedness::Signed, IntegerWidth::W16) => Self::Signed16(raw_bits as u16),
            (IntegerSignedness::Signed, IntegerWidth::W32) => Self::Signed32(raw_bits as u32),
            (IntegerSignedness::Signed, IntegerWidth::W64) => Self::Signed64(raw_bits),
            (IntegerSignedness::Unsigned, IntegerWidth::W8) => Self::Unsigned8(raw_bits as u8),
            (IntegerSignedness::Unsigned, IntegerWidth::W16) => Self::Unsigned16(raw_bits as u16),
            (IntegerSignedness::Unsigned, IntegerWidth::W32) => Self::Unsigned32(raw_bits as u32),
            (IntegerSignedness::Unsigned, IntegerWidth::W64) => Self::Unsigned64(raw_bits),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerUnaryOperator {
    Identity,
    Negate,
    Increment,
    Decrement,
    BitNot,
}

impl IntegerUnaryOperator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Negate => "negate",
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::BitNot => "bit-not",
        }
    }
}

/// A unary source-integer operation with an exact operand/result kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerUnaryOperation {
    kind: IntegerKind,
    operator: IntegerUnaryOperator,
}

impl IntegerUnaryOperation {
    pub const fn new(kind: IntegerKind, operator: IntegerUnaryOperator) -> Self {
        Self { kind, operator }
    }

    pub const fn kind(self) -> IntegerKind {
        self.kind
    }

    pub const fn operator(self) -> IntegerUnaryOperator {
        self.operator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerBinaryOperator {
    Add,
    Subtract,
    Multiply,
    BitAnd,
    BitOr,
    BitXor,
}

impl IntegerBinaryOperator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::BitAnd => "bit-and",
            Self::BitOr => "bit-or",
            Self::BitXor => "bit-xor",
        }
    }
}

/// A same-kind binary source-integer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerBinaryOperation {
    kind: IntegerKind,
    operator: IntegerBinaryOperator,
}

impl IntegerBinaryOperation {
    pub const fn new(kind: IntegerKind, operator: IntegerBinaryOperator) -> Self {
        Self { kind, operator }
    }

    pub const fn kind(self) -> IntegerKind {
        self.kind
    }

    pub const fn operator(self) -> IntegerBinaryOperator {
        self.operator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafeIntegerDivRemOperator {
    Divide,
    Remainder,
}

impl SafeIntegerDivRemOperator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Divide => "divide",
            Self::Remainder => "remainder",
        }
    }
}

/// A source integer division or remainder whose producer has emitted both
/// required safety guards: a nonzero divisor, plus `MIN / -1` separation for
/// signed kinds. LIR consumes this node as the structural proof that the
/// backend operation cannot trigger division poison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SafeIntegerDivRemOperation {
    kind: IntegerKind,
    operator: SafeIntegerDivRemOperator,
}

impl SafeIntegerDivRemOperation {
    pub const fn new(kind: IntegerKind, operator: SafeIntegerDivRemOperator) -> Self {
        Self { kind, operator }
    }

    pub const fn kind(self) -> IntegerKind {
        self.kind
    }

    pub const fn operator(self) -> SafeIntegerDivRemOperator {
        self.operator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerComparisonOperator {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

impl IntegerComparisonOperator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::LessThan => "less-than",
            Self::LessThanOrEqual => "less-than-or-equal",
            Self::GreaterThan => "greater-than",
            Self::GreaterThanOrEqual => "greater-than-or-equal",
            Self::Equal => "equal",
            Self::NotEqual => "not-equal",
        }
    }
}

/// A same-kind integer comparison whose result is always `Boolean`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerComparisonOperation {
    operand_kind: IntegerKind,
    operator: IntegerComparisonOperator,
}

impl IntegerComparisonOperation {
    pub const fn new(operand_kind: IntegerKind, operator: IntegerComparisonOperator) -> Self {
        Self {
            operand_kind,
            operator,
        }
    }

    pub const fn operand_kind(self) -> IntegerKind {
        self.operand_kind
    }

    pub const fn operator(self) -> IntegerComparisonOperator {
        self.operator
    }
}

/// The three-way integer comparison. Its result contract is canonical
/// `Long`, independently of the operand width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerCompareToOperation {
    operand_kind: IntegerKind,
}

impl IntegerCompareToOperation {
    pub const RESULT_KIND: IntegerKind = IntegerKind::SIGNED_64;

    pub const fn new(operand_kind: IntegerKind) -> Self {
        Self { operand_kind }
    }

    pub const fn operand_kind(self) -> IntegerKind {
        self.operand_kind
    }

    pub const fn result_kind(self) -> IntegerKind {
        Self::RESULT_KIND
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerShiftOperator {
    Left,
    /// Arithmetic for signed kinds and logical for unsigned kinds.
    Right,
    /// The signed-only `ushr` operation.
    UnsignedRight,
}

impl IntegerShiftOperator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::UnsignedRight => "unsigned-right",
        }
    }
}

/// An integer shift with an exact value kind. The source operation consumes a
/// `Long` count, but MIR stores only its already-masked and value-width-
/// normalized form so LIR can lower the instruction mechanically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerShiftOperation {
    value_kind: IntegerKind,
    operator: IntegerShiftOperator,
}

impl IntegerShiftOperation {
    /// Returns `None` only for the source-invalid unsigned `ushr` pairing.
    /// Unsigned `shr` is represented by `Right` and is already logical.
    pub const fn new(value_kind: IntegerKind, operator: IntegerShiftOperator) -> Option<Self> {
        if matches!(operator, IntegerShiftOperator::UnsignedRight)
            && matches!(value_kind.signedness(), IntegerSignedness::Unsigned)
        {
            None
        } else {
            Some(Self {
                value_kind,
                operator,
            })
        }
    }

    pub const fn value_kind(self) -> IntegerKind {
        self.value_kind
    }

    pub const fn count_kind(self) -> IntegerKind {
        self.value_kind
    }

    pub const fn operator(self) -> IntegerShiftOperator {
        self.operator
    }
}

/// A total, explicit conversion between two source integer kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerConversion {
    source_kind: IntegerKind,
    target_kind: IntegerKind,
}

impl IntegerConversion {
    pub const fn new(source_kind: IntegerKind, target_kind: IntegerKind) -> Self {
        Self {
            source_kind,
            target_kind,
        }
    }

    pub const fn source_kind(self) -> IntegerKind {
        self.source_kind
    }

    pub const fn target_kind(self) -> IntegerKind {
        self.target_kind
    }
}
