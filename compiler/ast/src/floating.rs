use std::fmt;

use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatSuffix {
    None,
    Float,
}

/// Unrounded decimal digits and exponent. Separators and suffix are removed
/// from `decimal`; a leading sign remains an ordinary unary expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatLiteralSyntax {
    pub decimal: String,
    pub suffix: FloatSuffix,
    pub span: Span,
}

impl fmt::Display for FloatLiteralSyntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.decimal)?;
        if self.suffix == FloatSuffix::Float {
            f.write_str("f")?;
        }
        Ok(())
    }
}
