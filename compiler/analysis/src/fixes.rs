use scoop_source::{SourceEdit, SourceLengthError, Span};
use scoop_syntax::{ExpectedSyntax, MissingSyntax, SyntaxKind};

use crate::SourceInput;

/// A concrete grammar repair. The index addresses this source version's missing item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxFix {
    pub missing: usize,
    pub edit: SourceEdit,
}

impl SourceInput {
    pub fn syntax_fixes(&self, range: Span) -> Result<Vec<SyntaxFix>, &SourceLengthError> {
        let parsed = self.parse()?;
        Ok(parsed
            .missing()
            .iter()
            .enumerate()
            .filter(|(_, missing)| range.start <= missing.offset && missing.offset <= range.end)
            .filter_map(|(index, missing)| self.missing_fix(index, missing))
            .collect())
    }

    pub fn syntax_fix(&self, index: usize) -> Result<Option<SyntaxFix>, &SourceLengthError> {
        Ok(self
            .parse()?
            .missing()
            .get(index)
            .and_then(|missing| self.missing_fix(index, missing)))
    }

    fn missing_fix(&self, index: usize, missing: &MissingSyntax) -> Option<SyntaxFix> {
        let ExpectedSyntax::Token(token) = missing.expected else {
            return None;
        };
        let replacement = match token {
            SyntaxKind::RParen => ")",
            SyntaxKind::RBrace => "}",
            SyntaxKind::RBracket => "]",
            SyntaxKind::Greater => ">",
            SyntaxKind::Comma => ",",
            SyntaxKind::Colon => ":",
            SyntaxKind::Semicolon => ";",
            SyntaxKind::Equal => "=",
            SyntaxKind::Arrow => "->",
            _ => return None,
        };
        Some(SyntaxFix {
            missing: index,
            edit: SourceEdit {
                source: self.version,
                span: Span::new(missing.offset, missing.offset),
                replacement: replacement.into(),
            },
        })
    }
}
