//! Names are observed at binding, before transparent aliases are expanded.

use super::*;
use crate::{aliases::SourceTypeAliasId, imports::lookup::TypeLookupTarget};

mod applications;
mod local_types;
mod record;
mod targets;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TypeTarget {
    Nominal(hir::SourceNominalId),
    Alias(SourceTypeAliasId),
    Parameter(hir::TypeParamId),
    Annotation(scoop_identity::PersistentAnnotationId),
    Imported(hir::ImportedTarget),
}

#[derive(Clone)]
struct TypeUse {
    target: TypeTarget,
    is_declaration: bool,
    imports: Vec<hir::SemanticSourceLocation>,
}

#[derive(Clone, Default)]
pub(crate) struct TypeObservation {
    uses: imbl::HashMap<(usize, ast::Span), TypeUse>,
    applications: imbl::HashMap<(usize, ast::Span), hir::TypeId>,
}

impl TypeObservation {
    pub(crate) fn finish(
        mut self,
        lowerer: &mut Lowerer,
    ) -> (
        Vec<hir::EditingTypeReference>,
        Vec<hir::EditingTypeApplication>,
    ) {
        for (&origin, reference) in &self.uses {
            if reference.is_declaration
                && let TypeTarget::Alias(id) = reference.target
                && let Some(ty) = lowerer.resolved_type_alias_target(id)
            {
                self.applications.insert(origin, ty);
            }
        }
        let mut references = self
            .uses
            .into_iter()
            .filter_map(|((file, span), reference)| {
                let target = lowerer.editing_type_identity(reference.target)?;
                Some(hir::EditingTypeReference {
                    target,
                    origin: hir::SemanticSourceLocation {
                        source: lowerer.source_identity(file).clone(),
                        span,
                    },
                    definition: lowerer.editing_type_origin(reference.target),
                    is_declaration: reference.is_declaration,
                    imports: reference.imports,
                })
            })
            .collect::<Vec<_>>();
        references.sort_by(|left, right| {
            (
                &left.origin.source,
                left.origin.span.start,
                left.origin.span.end,
            )
                .cmp(&(
                    &right.origin.source,
                    right.origin.span.start,
                    right.origin.span.end,
                ))
        });
        let applications = self
            .applications
            .into_iter()
            .map(|((file, span), ty)| hir::EditingTypeApplication {
                origin: hir::SemanticSourceLocation {
                    source: lowerer.source_identity(file).clone(),
                    span,
                },
                ty,
            })
            .collect();
        (references, applications)
    }
}

impl DeclarationAnalysis {
    pub fn type_reference(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<hir::EditingValue<Option<hir::EditingTypeReference>>, EditingCancelled> {
        use hir::{EditingUnavailable as U, EditingValue as V};
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if let Some(reference) = self
            .type_references
            .iter()
            .find(|reference| &reference.origin.source == source && reference.origin.span == span)
        {
            return Ok(V::Known(Some(reference.clone())));
        }
        let check = std::rc::Rc::new(is_cancelled);
        let functions = self
            .functions
            .iter()
            .filter(|function| {
                &function.name_origin.source == source
                    && function.span.start <= span.start
                    && span.end <= function.span.end
            })
            .map(|function| function.function)
            .collect::<Vec<_>>();
        for function in functions {
            let check = check.clone();
            match self.function_facts(function, move || check())? {
                V::Known(facts) => {
                    if let Some(reference) = facts.types.iter().find(|reference| {
                        &reference.origin.source == source && reference.origin.span == span
                    }) {
                        return Ok(V::Known(Some(reference.clone())));
                    }
                }
                V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
            }
        }
        Ok(V::Known(None))
    }
}
