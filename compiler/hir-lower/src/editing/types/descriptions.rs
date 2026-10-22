use super::*;
use hir::{DocumentationSymbol as Symbol, EditingTypeIdentity as Identity};
use hir::{EditingUnavailable as U, EditingValue as V};

mod declarations;
mod parameters;

impl DeclarationAnalysis {
    pub fn type_description(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::EditingTypeDescription>>, EditingCancelled> {
        let check = std::rc::Rc::new(is_cancelled);
        let cancellation = check.clone();
        let (target, definition, application) =
            match self.type_reference(source, span, move || cancellation())? {
                V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
                V::Known(Some(reference)) => {
                    let application = if reference.is_declaration {
                        None
                    } else {
                        let cancellation = check.clone();
                        match self.type_application(source, span, move || cancellation())? {
                            V::Known(application) => application,
                            V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
                        }
                    };
                    (reference.target, reference.definition, application)
                }
                V::Known(None) => {
                    let bindings = self
                        .import_bindings(source, span)
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut selected = None;
                    for binding in bindings {
                        if check() {
                            return Err(EditingCancelled);
                        }
                        let Some(target) = self.lowerer.editing_import_type_target(&binding) else {
                            continue;
                        };
                        let definition = self.lowerer.editing_type_origin(target);
                        let Some(target) = self.lowerer.editing_type_identity(target) else {
                            return Ok(V::Unavailable(U::SignatureFailure));
                        };
                        if selected
                            .as_ref()
                            .is_some_and(|(previous, _)| *previous != target)
                        {
                            return Ok(V::Known(None));
                        }
                        selected = Some((target, definition));
                    }
                    let Some((target, definition)) = selected else {
                        return Ok(V::Known(None));
                    };
                    (target, definition, None)
                }
            };
        // Loading declaration binders must not select executable dependency uses
        // or retain query-only type applications in the source analysis.
        let mut query = self.lowerer.clone();
        query.in_semantic_query = true;
        let description = query.editing_type_declaration(target);
        if check() {
            return Err(EditingCancelled);
        }
        let Some((label, mut documentation)) = description else {
            return Ok(V::Unavailable(U::SignatureFailure));
        };
        if let Identity::Parameter(id) = target {
            documentation = self.documentation.as_ref().and_then(|docs| {
                docs.declarations
                    .iter()
                    .find_map(|declaration| match declaration.target {
                        hir::DocumentationTarget::TypeParameter { parameter, .. }
                            if parameter == id =>
                        {
                            Some(Symbol::Current(declaration.target))
                        }
                        _ => None,
                    })
            });
        }
        Ok(V::Known(Some(hir::EditingTypeDescription {
            target,
            label,
            application,
            documentation,
            definition,
        })))
    }
}

impl Lowerer {
    fn editing_type_declaration(&mut self, target: Identity) -> Option<(String, Option<Symbol>)> {
        match target {
            Identity::Nominal(id) => self.editing_nominal_declaration(id),
            Identity::Alias(id) => self.editing_alias_declaration(id),
            Identity::Parameter(id) => self.editing_parameter_label(id).map(|label| (label, None)),
            Identity::Annotation(id) => self.editing_annotation_declaration(id),
        }
    }
}

fn generics(parameters: &[hir::TypeParamDecl]) -> String {
    if parameters.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
