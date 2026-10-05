use super::*;
use crate::types::ResolvedTypeName;
use std::collections::HashSet;

impl Lowerer {
    pub(crate) fn resolve_annotation_declarations(&mut self) {
        let declarations = self
            .source_annotations
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for input in declarations {
            self.current_file = input.file;
            self.current_owner = input.owner;
            self.type_params_in_scope.clear();
            for annotation in &input.declaration.annotations {
                self.error(
                    annotation.span,
                    "annotation classes are not annotation targets".into(),
                );
            }
            let mut parameters = Vec::new();
            let mut names = HashSet::new();
            for parameter in &input.declaration.parameters {
                if !names.insert(parameter.name.text.clone()) {
                    self.error(
                        parameter.name.span,
                        format!("duplicate annotation parameter `{}`", parameter.name.text),
                    );
                    continue;
                }
                let Some(value_type) = self.resolve_type_ref(&parameter.ty) else {
                    continue;
                };
                if self.annotation_scalar_kind(value_type).is_none() {
                    self.error(parameter.ty.span, "annotation parameter types are limited to Boolean, String, Char and fixed-width integers".into());
                    continue;
                }
                let default = if let Some(default) = &parameter.default {
                    let Some(value) = self.annotation_constant(default, value_type, parameter.span)
                    else {
                        continue;
                    };
                    Some(value)
                } else {
                    None
                };
                parameters.push(hir::SourceAnnotationParameter {
                    name: parameter.name.text.clone(),
                    value_type,
                    default,
                });
            }
            if parameters.len() == input.declaration.parameters.len() {
                self.annotation_metadata
                    .declarations
                    .push(hir::SourceAnnotationDeclaration {
                        identity: input.identity,
                        parameters,
                        visibility: input.access.declared,
                        definition_origin: self.definition_origin(input.declaration.span),
                    });
            }
        }
        self.current_owner = None;
    }

    fn annotation_parameters(
        &mut self,
        id: PersistentAnnotationId,
        span: ast::Span,
    ) -> Option<Vec<hir::SourceAnnotationParameter>> {
        if self.source_annotations.contains_key(&id) {
            return self
                .annotation_metadata
                .declarations
                .iter()
                .find(|declaration| declaration.identity.id() == id)
                .map(|declaration| declaration.parameters.clone());
        }
        let declaration = self
            .dependencies
            .as_ref()?
            .annotation_declaration(id)?
            .declaration
            .clone();
        declaration
            .parameters
            .into_iter()
            .map(|parameter| {
                let value_type = match self.imported_signature_type(
                    &scoop_identity::SignatureTypeKey::Nominal(parameter.value_type),
                ) {
                    Ok(ty) => ty,
                    Err(error) => {
                        self.error(span, error.diagnostic("annotation parameter"));
                        return None;
                    }
                };
                Some(hir::SourceAnnotationParameter {
                    name: parameter.name.as_str().into(),
                    value_type,
                    default: parameter.default,
                })
            })
            .collect()
    }

    pub(crate) fn bind_source_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[ast::Annotation],
    ) {
        let mut applications = Vec::new();
        let mut seen = HashSet::new();
        for annotation in annotations {
            if crate::annotations::is_core_annotation(&annotation.name.text) {
                if matches!(
                    target,
                    hir::SourceAnnotationTarget::Field(_)
                        | hir::SourceAnnotationTarget::Variant(_)
                        | hir::SourceAnnotationTarget::VariantField(_)
                ) {
                    self.error(
                        annotation.span,
                        format!(
                            "`@{}` is not allowed on a field or enum variant",
                            annotation.name.text
                        ),
                    );
                }
                continue;
            }
            let path = annotation
                .name
                .text
                .split('.')
                .map(|name| ast::Ident {
                    text: name.into(),
                    span: annotation.name.span,
                })
                .collect::<Vec<_>>();
            let id = match self.resolve_type_name_path(&path, false) {
                Ok(Some(ResolvedTypeName::Annotation(id))) => id,
                Ok(Some(_)) => {
                    self.error(
                        annotation.name.span,
                        format!("`{}` is not an annotation class", annotation.name.text),
                    );
                    continue;
                }
                Ok(None) => {
                    self.error(
                        annotation.name.span,
                        format!("unknown annotation `{}`", annotation.name.text),
                    );
                    continue;
                }
                Err(()) => continue,
            };
            if !seen.insert(id) {
                self.error(
                    annotation.span,
                    format!(
                        "annotation `@{}` must not be repeated on the same target",
                        annotation.name.text
                    ),
                );
                continue;
            }
            let Some(parameters) = self.annotation_parameters(id, annotation.span) else {
                continue;
            };
            if let Some(arguments) = self.bind_annotation_arguments(annotation, &parameters) {
                applications.push(hir::SourceAnnotationApplication {
                    annotation: id,
                    arguments,
                    definition_origin: self.definition_origin(annotation.span),
                });
            }
        }
        self.validate_serialization_annotations(target, &applications);
        if !applications.is_empty() {
            self.annotation_metadata
                .targets
                .push(hir::SourceAnnotatedTarget {
                    target,
                    annotations: applications,
                });
        }
    }

    fn bind_annotation_arguments(
        &mut self,
        annotation: &ast::Annotation,
        parameters: &[hir::SourceAnnotationParameter],
    ) -> Option<Vec<hir::CanonicalConstValueV1>> {
        let mut values = vec![None; parameters.len()];
        let mut used = HashSet::new();
        let mut failed = false;
        for (position, argument) in annotation.args.iter().enumerate() {
            let index = match &argument.name {
                Some(name) => parameters
                    .iter()
                    .position(|parameter| parameter.name == name.text),
                None => (position < parameters.len()).then_some(position),
            };
            let Some(index) = index else {
                self.error(
                    argument.span,
                    match &argument.name {
                        Some(name) => format!("unknown annotation parameter `{}`", name.text),
                        None => "too many annotation arguments".into(),
                    },
                );
                failed = true;
                continue;
            };
            if !used.insert(index) {
                self.error(
                    argument.span,
                    format!(
                        "annotation parameter `{}` is supplied more than once",
                        parameters[index].name
                    ),
                );
                failed = true;
                continue;
            }
            values[index] = self.annotation_constant(
                &argument.value,
                parameters[index].value_type,
                argument.span,
            );
            failed |= values[index].is_none();
        }
        for (index, parameter) in parameters.iter().enumerate() {
            if used.contains(&index) {
                continue;
            }
            values[index] = parameter.default.clone();
            if values[index].is_none() {
                self.error(
                    annotation.span,
                    format!("missing annotation argument `{}`", parameter.name),
                );
                failed = true;
            }
        }
        if failed {
            None
        } else {
            values.into_iter().collect()
        }
    }
}
