use encoding_rs::Encoding;

#[derive(Clone, Copy)]
pub enum Kind {
    Ascii,
    Latin1,
    Utf8,
    Utf16(u8),
    Utf32(u8),
    Rs(&'static Encoding),
}

impl Kind {
    pub fn ambiguous(self) -> bool {
        matches!(self, Self::Utf16(0) | Self::Utf32(0))
    }

    pub fn ordered(self, order: u8) -> Self {
        match self {
            Self::Utf16(0) => Self::Utf16(order),
            Self::Utf32(0) => Self::Utf32(order),
            _ => self,
        }
    }

    pub fn bom(self, order: u8) -> &'static [u8] {
        match self.ordered(order) {
            Self::Utf8 => b"\xef\xbb\xbf",
            Self::Utf16(1) => b"\xfe\xff",
            Self::Utf16(2) => b"\xff\xfe",
            Self::Utf32(1) => b"\0\0\xfe\xff",
            Self::Utf32(2) => b"\xff\xfe\0\0",
            _ => b"",
        }
    }

    pub fn bom_pair(self) -> (&'static [u8], &'static [u8], u8) {
        match self {
            Self::Utf8 => (Self::Utf8.bom(1), Self::Utf8.bom(1), 0),
            Self::Utf16(order) => (Self::Utf16(1).bom(1), Self::Utf16(2).bom(2), order),
            Self::Utf32(order) => (Self::Utf32(1).bom(1), Self::Utf32(2).bom(2), order),
            _ => (b"", b"", 0),
        }
    }
}
