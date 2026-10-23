use crate::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScoopLanguage {}

impl rowan::Language for ScoopLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw.0)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<ScoopLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<ScoopLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<ScoopLanguage>;
