use super::*;
use hir::AnnotationParameterType;
use std::collections::HashSet;

impl Lowerer {
    pub(crate) fn resolve_annotation_declarations(&mut self) {
        let ids = self.source_annotations.keys().copied().collect::<Vec<_>>();
        self.resolve_annotation_group(&ids);
    }

    pub(crate) fn resolve_local_owner_annotations(&mut self, owners: &[Owner]) {
        let ids = self
            .source_annotations
            .iter()
            .filter_map(|(&id, input)| {
                input
                    .owner
                    .is_some_and(|o| owners.contains(&o))
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        self.resolve_annotation_group(&ids);
    }

    fn resolve_annotation_group(&mut self, ids: &[PersistentAnnotationId]) {
        for &id in ids {
            self.annotation_parameters(id, self.source_annotations[&id].declaration.span);
        }
        for &id in ids {
            let input = self.source_annotations[&id].clone();
            self.current_file = input.file;
            self.current_owner = input.owner;
            self.bind_annotations(
                hir::SourceAnnotationTarget::Annotation(id),
                &input.declaration.annotations,
                false,
            );
        }
        self.bind_annotation_declaration_rules(ids);
    }

    pub(super) fn annotation_parameters(
        &mut self,
        id: PersistentAnnotationId,
        span: ast::Span,
    ) -> Option<Vec<hir::SourceAnnotationParameter>> {
        let Some(input) = self.source_annotations.get(&id).cloned() else {
            return self.imported_annotation_parameters(id, span);
        };
        match self.annotation_resolution.get(&id) {
            Some(AnnotationResolution::Resolved) => {
                return self
                    .annotation_metadata
                    .declarations
                    .iter()
                    .find(|d| d.identity.id() == id)
                    .map(|d| d.parameters.clone());
            }
            Some(AnnotationResolution::Failed) => return None,
            Some(AnnotationResolution::Resolving) => {
                self.error(
                    span,
                    "recursive annotation parameter or default dependency".into(),
                );
                return None;
            }
            None => {}
        }
        self.annotation_resolution
            .insert(id, AnnotationResolution::Resolving);
        let file = std::mem::replace(&mut self.current_file, input.file);
        let owner = std::mem::replace(&mut self.current_owner, input.owner);
        let type_parameters = std::mem::take(&mut self.type_params_in_scope);
        let mut parameters = Vec::new();
        let mut names = HashSet::new();
        for (index, parameter) in input.declaration.parameters.iter().enumerate() {
            if !names.insert(&parameter.name.text) {
                self.error(
                    parameter.name.span,
                    format!("duplicate annotation parameter `{}`", parameter.name.text),
                );
                continue;
            }
            let Some(value_type) = self.resolve_annotation_parameter_type(&parameter.ty) else {
                continue;
            };
            self.annotate_type_site(
                hir::SourceAnnotationTarget::Annotation(id),
                hir::AnnotationTypeSlot::AnnotationParameter(index as u32),
                &parameter.ty,
            );
            let nested = match value_type {
                AnnotationParameterType::Annotation(id)
                | AnnotationParameterType::AnnotationArray { element: id, .. } => Some(id),
                AnnotationParameterType::Value(_) => None,
            };
            if let Some(id) = nested
                && self.annotation_parameters(id, parameter.ty.span).is_none()
            {
                continue;
            }
            let default = match &parameter.default {
                Some(value) => {
                    let Some(value) = self.annotation_value(value, value_type, parameter.span)
                    else {
                        continue;
                    };
                    Some(value)
                }
                None => None,
            };
            parameters.push(hir::SourceAnnotationParameter {
                name: parameter.name.text.clone(),
                value_type,
                default,
            });
        }
        let complete = parameters.len() == input.declaration.parameters.len();
        if complete {
            self.document_annotation(id);
            let rules = self.initial_annotation_rules(id);
            self.annotation_metadata
                .declarations
                .push(hir::SourceAnnotationDeclaration {
                    identity: input.identity,
                    parameters: parameters.clone(),
                    rules,
                    visibility: input.access.declared,
                    definition_origin: self.definition_origin(input.declaration.span),
                });
        }
        self.current_file = file;
        self.current_owner = owner;
        self.type_params_in_scope = type_parameters;
        self.annotation_resolution.insert(
            id,
            if complete {
                AnnotationResolution::Resolved
            } else {
                AnnotationResolution::Failed
            },
        );
        complete.then_some(parameters)
    }

    fn imported_annotation_parameters(
        &mut self,
        id: PersistentAnnotationId,
        span: ast::Span,
    ) -> Option<Vec<hir::SourceAnnotationParameter>> {
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
                let value_type = match parameter.value_type {
                    AnnotationParameterType::Value(ty) => match self.imported_signature_type(&ty) {
                        Ok(ty) => AnnotationParameterType::Value(ty),
                        Err(error) => {
                            self.error(span, error.diagnostic("annotation parameter"));
                            return None;
                        }
                    },
                    AnnotationParameterType::Annotation(id) => {
                        AnnotationParameterType::Annotation(id)
                    }
                    AnnotationParameterType::AnnotationArray { array, element } => {
                        AnnotationParameterType::AnnotationArray { array, element }
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
}
