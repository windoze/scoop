use scoop_source::{Diagnostic, DiagnosticSeverity};

use crate::{GreenNode, SyntaxKind, SyntaxNode};

/// An expected syntax item, separate from the human diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedSyntax {
    Token(SyntaxKind),
    Name,
    Type,
    Expression,
    Pattern,
    Description(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSyntax {
    pub offset: u32,
    pub expected: ExpectedSyntax,
}

/// One grammar run: immutable concrete syntax and its strict AST projection.
/// The green tree can cross worker boundaries; red nodes are created on demand.
#[derive(Debug, Clone)]
pub struct Parse<T> {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
    missing: Vec<MissingSyntax>,
    ast: Option<T>,
}

impl<T> Parse<T> {
    pub fn new(
        green: GreenNode,
        diagnostics: Vec<Diagnostic>,
        missing: Vec<MissingSyntax>,
        ast: Option<T>,
    ) -> Self {
        let ast = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            None
        } else {
            ast
        };
        Self {
            green,
            diagnostics,
            missing,
            ast,
        }
    }

    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn missing(&self) -> &[MissingSyntax] {
        &self.missing
    }

    pub fn ast(&self) -> Option<&T> {
        self.ast.as_ref()
    }

    pub fn into_ast(self) -> Result<T, Vec<Diagnostic>> {
        self.ast.ok_or(self.diagnostics)
    }
}
