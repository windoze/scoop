/// Reads and copies of the String and byte-buffer representations.
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
    AllocateZeroed,
    ByteAtUnchecked,
    SetByteUnchecked,
    FromBytePrefixUnchecked,
    CopyToBytesUnchecked,
    CopyBytePrefixUnchecked,
    CopyRawBytesUnchecked,
}

impl StringIntrinsic {
    pub const ALL: [Self; 15] = [
        Self::ByteLength,
        Self::ByteAt,
        Self::CharacterAtByte,
        Self::CopyUtf8Unchecked,
        Self::SliceBytes,
        Self::FromBytes,
        Self::ToBytes,
        Self::ToMutableBytes,
        Self::AllocateZeroed,
        Self::ByteAtUnchecked,
        Self::SetByteUnchecked,
        Self::FromBytePrefixUnchecked,
        Self::CopyToBytesUnchecked,
        Self::CopyBytePrefixUnchecked,
        Self::CopyRawBytesUnchecked,
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
            Self::AllocateZeroed => "string_allocate_zeroed",
            Self::ByteAtUnchecked => "string_byte_at_unchecked",
            Self::SetByteUnchecked => "string_set_byte_unchecked",
            Self::FromBytePrefixUnchecked => "string_from_byte_prefix_unchecked",
            Self::CopyToBytesUnchecked => "string_copy_to_bytes_unchecked",
            Self::CopyBytePrefixUnchecked => "bytes_copy_prefix_unchecked",
            Self::CopyRawBytesUnchecked => "bytes_copy_raw_unchecked",
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
            Self::AllocateZeroed => 9,
            Self::ByteAtUnchecked => 10,
            Self::SetByteUnchecked => 11,
            Self::FromBytePrefixUnchecked => 12,
            Self::CopyToBytesUnchecked => 13,
            Self::CopyBytePrefixUnchecked => 14,
            Self::CopyRawBytesUnchecked => 15,
        }
    }
}
