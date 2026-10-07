/// Synchronous core borrows whose callback keeps the backing object pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataBorrowIntrinsic {
    Array,
    MutableArray,
    String,
}

impl DataBorrowIntrinsic {
    pub const ALL: [Self; 3] = [Self::Array, Self::MutableArray, Self::String];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Array => "array_with_data_pointer",
            Self::MutableArray => "mutable_array_with_data_pointer",
            Self::String => "string_with_utf8_bytes",
        }
    }

    pub const fn wire_tag(self) -> u64 {
        match self {
            Self::Array => 1,
            Self::MutableArray => 2,
            Self::String => 3,
        }
    }

    pub const fn type_parameter_count(self) -> u32 {
        match self {
            Self::Array | Self::MutableArray => 2,
            Self::String => 1,
        }
    }
}
