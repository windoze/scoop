//! 单源文件内的字节偏移区间。

use std::fmt;

/// UTF-8 字节偏移区间（半开 `[start, end)`），相对于某一个 [`crate::SourceFile`]。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// 零长 span，指向某个偏移位置（常用于“在此处缺少某成分”的诊断）。
    pub fn point(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// 合成节点（由编译器生成、不对应任何源文本）使用的空 span。
    pub fn synthetic() -> Self {
        Self::new(0, 0)
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// 返回能同时覆盖 `self` 与 `other` 的最小 span。
    pub fn join(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_len_saturates_when_reversed() {
        assert_eq!(Span::new(2, 5).len(), 3);
        assert_eq!(Span::new(5, 2).len(), 0);
    }

    #[test]
    fn span_join_covers_both() {
        assert_eq!(Span::new(1, 3).join(Span::new(2, 8)), Span::new(1, 8));
        assert_eq!(Span::new(5, 6).join(Span::new(0, 2)), Span::new(0, 6));
    }

    #[test]
    fn span_debug_matches_byte_range() {
        assert_eq!(format!("{:?}", Span::new(4, 9)), "4..9");
    }
}
