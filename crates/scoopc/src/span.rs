//! 源码 span（字节偏移区间）。
//!
//! Scoop 编译器内部统一使用 UTF-8 字节偏移作为 span 计量单位。
//! 这在编译器内部最简单、最稳定；向 LSP/IDE 暴露时可以再做 UTF-16 转换。

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn synthetic_prelude() -> Self {
        Self::new(0, 0)
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        (span.start, span.len()).into()
    }
}
