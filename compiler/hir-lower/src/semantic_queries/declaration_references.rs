//! Resolve typed references against actual declarations, including empty targets.

use super::*;
use hir::{AnnotationTargetV1 as Target, SourceAnnotationTarget as Source};
use std::collections::HashMap;

mod callables;
mod nominals;
mod properties;

#[derive(Default)]
pub(super) struct DeclarationReferences {
    declarations: HashMap<hir::DecodedAnnotationTargetV1, hir::SemanticDeclaration>,
}

impl FrontendQueries<'_, '_> {
    pub(super) fn query_declaration(
        &mut self,
        reference: hir::DecodedAnnotationTargetV1,
    ) -> Result<Option<hir::SemanticDeclaration>, Diagnostic> {
        let declarations = self
            .declaration_references
            .get_or_init(|| DeclarationReferences::new(self.lowerer, self.body_queries.data()?));
        declarations
            .as_ref()
            .map(|index| index.declarations.get(&reference).cloned())
            .map_err(Clone::clone)
    }
}

impl DeclarationReferences {
    fn new(
        lowerer: &mut Lowerer,
        data: &body_projection::ProjectionData,
    ) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        index.nominals(lowerer);
        index.callables(lowerer, data);
        index.properties(lowerer, data);
        let aliases = lowerer
            .type_aliases
            .iter()
            .map(|(id, alias)| (id, alias.origin))
            .collect::<Vec<_>>();
        for (id, origin) in aliases {
            let identity = lowerer.query_alias_identity(id)?.id();
            index.current(
                Target::TypeAlias(identity),
                Source::TypeAlias(id),
                source_location(lowerer, origin),
            );
        }
        for declaration in &lowerer.annotation_metadata.declarations {
            let id = declaration.identity.id();
            index.current(
                Target::Annotation(id),
                Source::Annotation(id),
                source_location(lowerer, declaration.definition_origin),
            );
        }
        if let Some(dependencies) = &lowerer.dependencies {
            for (reference, origin) in dependencies.declaration_locations() {
                index
                    .declarations
                    .entry(reference.into())
                    .or_insert(hir::SemanticDeclaration {
                        reference,
                        annotations: hir::SemanticAnnotationTarget::Imported(reference),
                        origin: imported_location(origin.origin())?,
                    });
            }
        }
        Ok(index)
    }

    fn current(&mut self, reference: Target, target: Source, origin: hir::SemanticSourceLocation) {
        self.declarations.insert(
            reference.into(),
            hir::SemanticDeclaration {
                reference,
                annotations: hir::SemanticAnnotationTarget::Current(target),
                origin,
            },
        );
    }
}

fn source_location(
    lowerer: &Lowerer,
    origin: hir::DefinitionOrigin,
) -> hir::SemanticSourceLocation {
    hir::SemanticSourceLocation {
        source: lowerer.source_identity(origin.file as usize).clone(),
        span: origin.span,
    }
}
