//! Bound documentation can be displayed without owning a successful HIR module.

use std::collections::HashMap;

use super::*;
use crate::SemanticSourceLocation;

#[derive(Clone, Debug)]
pub struct SemanticDocumentation {
    pub declarations: Vec<SemanticDocumentationDeclaration>,
    pub source_locations: HashMap<DocumentationSymbol, SemanticSourceLocation>,
    pub warnings: Vec<Diagnostic>,
    pub exported_targets: HashMap<AnnotationTargetV1, DocumentationTarget>,
}

#[derive(Clone, Debug)]
pub struct SemanticDocumentationDeclaration {
    pub target: DocumentationTarget,
    pub comment: Option<DocumentationComment>,
}

/// Direct comments and descriptions attached by a tag retain distinct syntax.
pub enum DocumentationContent<'a> {
    Comment(&'a DocumentationComment),
    Markdown(&'a DocumentationMarkdown),
}

impl SemanticDocumentation {
    pub fn from_complete(
        documentation: ConeDocumentation,
        export: &crate::ExportHir,
    ) -> Result<Self, crate::AnnotationProductionError> {
        let exported_targets = documentation
            .declarations
            .iter()
            .filter_map(|entry| {
                let DocumentationTarget::Declaration(target) = entry.target else {
                    return None;
                };
                Some(
                    AnnotationTargetV1::from_source(export, target)
                        .map(|reference| (reference, entry.target)),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            declarations: documentation
                .declarations
                .into_iter()
                .map(|entry| SemanticDocumentationDeclaration {
                    target: entry.target,
                    comment: entry.comment,
                })
                .collect(),
            source_locations: documentation.source_locations,
            warnings: documentation.warnings,
            exported_targets,
        })
    }

    /// Read an actual bound declaration, including a source-provider identity.
    pub fn get(&self, symbol: DocumentationSymbol) -> Option<DocumentationContent<'_>> {
        let target = match symbol {
            DocumentationSymbol::Current(target) => target,
            DocumentationSymbol::Imported(target) => *self.exported_targets.get(&target)?,
            DocumentationSymbol::ImportedDerivedEquality(_) => return None,
        };
        if let Some(comment) = self
            .declarations
            .iter()
            .find(|entry| entry.target == target)
            .and_then(|entry| entry.comment.as_ref())
        {
            return Some(DocumentationContent::Comment(comment));
        }
        self.declarations
            .iter()
            .filter_map(|entry| entry.comment.as_ref())
            .flat_map(|comment| &comment.tags)
            .find_map(|tag| {
                let destination = match tag.kind {
                    DocumentationTagKind::Param(target)
                    | DocumentationTagKind::Property(target)
                    | DocumentationTagKind::Constructor(target)
                    | DocumentationTagKind::Receiver(target) => target,
                    _ => return None,
                };
                (destination == target).then_some(DocumentationContent::Markdown(&tag.body))
            })
    }
}
