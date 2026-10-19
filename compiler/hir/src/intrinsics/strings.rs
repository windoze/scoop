/// Bounded reads of the existing String representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StringIntrinsic {
    ByteLength,
    ByteAt,
    CharacterAtByte,
}

impl StringIntrinsic {
    pub const ALL: [Self; 3] = [Self::ByteLength, Self::ByteAt, Self::CharacterAtByte];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ByteLength => "string_byte_length",
            Self::ByteAt => "string_byte_at",
            Self::CharacterAtByte => "string_character_at_byte",
        }
    }

    pub const fn wire_tag(self) -> u64 {
        match self {
            Self::ByteLength => 1,
            Self::ByteAt => 2,
            Self::CharacterAtByte => 3,
        }
    }
}
