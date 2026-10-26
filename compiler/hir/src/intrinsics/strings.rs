/// Reads and copies of the existing String representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StringIntrinsic {
    ByteLength,
    ByteAt,
    CharacterAtByte,
    CopyUtf8Unchecked,
    SliceBytes,
    FromBytes,
    ToBytes,
    ToMutableBytes,
}

impl StringIntrinsic {
    pub const ALL: [Self; 8] = [
        Self::ByteLength,
        Self::ByteAt,
        Self::CharacterAtByte,
        Self::CopyUtf8Unchecked,
        Self::SliceBytes,
        Self::FromBytes,
        Self::ToBytes,
        Self::ToMutableBytes,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ByteLength => "string_byte_length",
            Self::ByteAt => "string_byte_at",
            Self::CharacterAtByte => "string_character_at_byte",
            Self::CopyUtf8Unchecked => "string_copy_utf8_unchecked",
            Self::SliceBytes => "string_slice_bytes",
            Self::FromBytes => "string_from_bytes",
            Self::ToBytes => "string_to_bytes",
            Self::ToMutableBytes => "string_to_mutable_bytes",
        }
    }

    pub const fn wire_tag(self) -> u64 {
        match self {
            Self::ByteLength => 1,
            Self::ByteAt => 2,
            Self::CharacterAtByte => 3,
            Self::CopyUtf8Unchecked => 4,
            Self::SliceBytes => 5,
            Self::FromBytes => 6,
            Self::ToBytes => 7,
            Self::ToMutableBytes => 8,
        }
    }
}
