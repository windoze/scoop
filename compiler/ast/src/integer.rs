use std::fmt;

use crate::Span;

/// The source radix of an integer literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerRadix {
    Decimal,
    Binary,
    Hexadecimal,
}

/// The source suffix of an integer literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerSuffix {
    None,
    Unsigned,
    Long,
    UnsignedLong,
}

/// Integer syntax retained until HIR commits the literal to an exact type.
///
/// `magnitude` is always non-negative. A leading `-` remains a separate unary
/// expression so HIR can apply the signed-minimum boundary rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerLiteralSyntax {
    pub magnitude: u64,
    pub radix: IntegerRadix,
    pub suffix: IntegerSuffix,
    pub span: Span,
}

impl fmt::Display for IntegerLiteralSyntax {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.radix {
            IntegerRadix::Decimal => write!(formatter, "{}", self.magnitude)?,
            IntegerRadix::Binary => write!(formatter, "0b{:b}", self.magnitude)?,
            IntegerRadix::Hexadecimal => write!(formatter, "0x{:x}", self.magnitude)?,
        }
        match self.suffix {
            IntegerSuffix::None => Ok(()),
            IntegerSuffix::Unsigned => formatter.write_str("u"),
            IntegerSuffix::Long => formatter.write_str("L"),
            IntegerSuffix::UnsignedLong => formatter.write_str("uL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_preserves_semantic_radix_and_suffix() {
        let span = Span::new(3, 9);
        assert_eq!(
            IntegerLiteralSyntax {
                magnitude: 42,
                radix: IntegerRadix::Decimal,
                suffix: IntegerSuffix::None,
                span,
            }
            .to_string(),
            "42"
        );
        assert_eq!(
            IntegerLiteralSyntax {
                magnitude: 42,
                radix: IntegerRadix::Binary,
                suffix: IntegerSuffix::Unsigned,
                span,
            }
            .to_string(),
            "0b101010u"
        );
        assert_eq!(
            IntegerLiteralSyntax {
                magnitude: 255,
                radix: IntegerRadix::Hexadecimal,
                suffix: IntegerSuffix::UnsignedLong,
                span,
            }
            .to_string(),
            "0xffuL"
        );
    }
}
