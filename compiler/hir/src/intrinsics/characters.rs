//! The scalar operations needed by the ordinary Char source declaration.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CharIntrinsic {
    Code,
    FromCodeUnchecked,
    Equals,
    CompareTo,
}

impl CharIntrinsic {
    pub const ALL: [Self; 4] = [
        Self::Code,
        Self::FromCodeUnchecked,
        Self::Equals,
        Self::CompareTo,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Code => "char_code",
            Self::FromCodeUnchecked => "char_from_code_unchecked",
            Self::Equals => "char_equals",
            Self::CompareTo => "char_compare_to",
        }
    }
    pub(crate) const fn wire_tag(self) -> u64 {
        match self {
            Self::Code => 1,
            Self::FromCodeUnchecked => 2,
            Self::Equals => 3,
            Self::CompareTo => 4,
        }
    }
}
