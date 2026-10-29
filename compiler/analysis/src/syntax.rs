use scoop_source::{SourceVersion, Span};
use scoop_syntax::{SyntaxKind, SyntaxNode, TextSize, TokenAtOffset};

/// A syntax location is meaningful only in its recorded source version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxPtr {
    pub source: SourceVersion,
    pub kind: SyntaxKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    Region,
    Comment,
    Imports,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    pub span: Span,
    pub kind: FoldKind,
}

pub fn folding_ranges(root: &SyntaxNode) -> Vec<Fold> {
    let mut folds = Vec::new();
    for node in root.descendants() {
        if node.kind() != SyntaxKind::Statement {
            import_folds(&node, &mut folds);
        }
        let range = node.text_range();
        let span = Span::new(range.start().into(), range.end().into());
        match node.kind() {
            SyntaxKind::Block
            | SyntaxKind::ClassBody
            | SyntaxKind::StructBody
            | SyntaxKind::InterfaceBody
            | SyntaxKind::InterfaceImplementation
            | SyntaxKind::EnumDecl
            | SyntaxKind::WhenExpr
            | SyntaxKind::LambdaExpr
            | SyntaxKind::ArgumentList
            | SyntaxKind::ParameterList
            | SyntaxKind::ArrayExpr
            | SyntaxKind::AnnotationArray => folds.push(Fold {
                span,
                kind: FoldKind::Region,
            }),
            _ => {}
        }
    }
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if matches!(
            token.kind(),
            SyntaxKind::BlockComment | SyntaxKind::DocBlockComment
        ) {
            let range = token.text_range();
            folds.push(Fold {
                span: Span::new(range.start().into(), range.end().into()),
                kind: FoldKind::Comment,
            });
        }
    }
    folds.sort_by_key(|fold| (fold.span.start, fold.span.end));
    folds.dedup_by_key(|fold| fold.span);
    folds
}

fn import_folds(parent: &SyntaxNode, folds: &mut Vec<Fold>) {
    let mut imports: Option<Span> = None;
    for child in parent.children() {
        let import = match child.kind() {
            SyntaxKind::ImportHeader => Some(child),
            SyntaxKind::Statement => child
                .children()
                .find(|node| node.kind() == SyntaxKind::ImportHeader),
            _ => None,
        };
        if let Some(import) = import {
            let range = import.text_range();
            imports
                .get_or_insert(Span::new(range.start().into(), range.end().into()))
                .end = range.end().into();
        } else if let Some(span) = imports.take() {
            folds.push(Fold {
                span,
                kind: FoldKind::Imports,
            });
        }
    }
    if let Some(span) = imports {
        folds.push(Fold {
            span,
            kind: FoldKind::Imports,
        });
    }
}

/// Smallest first. Adjacent tokens remain usable at EOF and between tokens.
pub fn selection_ranges(root: &SyntaxNode, offset: u32) -> Vec<Span> {
    let offset = TextSize::from(offset);
    if offset > root.text_range().end() {
        return Vec::new();
    }
    let token = match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => Some(token),
        TokenAtOffset::Between(left, right) => Some(if right.kind().is_trivia() {
            left
        } else {
            right
        }),
    };
    let mut ranges = Vec::new();
    if let Some(token) = token {
        if !token.kind().is_trivia() {
            ranges.push(token.text_range());
        }
        ranges.extend(token.parent_ancestors().map(|node| node.text_range()));
    } else {
        ranges.push(root.text_range());
    }
    ranges.dedup();
    ranges
        .into_iter()
        .map(|range| Span::new(range.start().into(), range.end().into()))
        .collect()
}
