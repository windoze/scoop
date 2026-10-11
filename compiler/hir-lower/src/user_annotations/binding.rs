use super::*;
use crate::types::ResolvedTypeName;
use std::collections::HashSet;

impl Lowerer {
    pub(crate) fn bind_source_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[ast::Annotation],
    ) {
        let mut applications = Vec::new();
        let mut seen = HashSet::new();
        for annotation in annotations {
            if annotation
                .name
                .single_text()
                .is_some_and(crate::annotations::is_core_annotation)
            {
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
                            annotation.name
                        ),
                    );
                }
                continue;
            }
            let path = annotation.name.segments().cloned().collect::<Vec<_>>();
            let id = match self.resolve_type_name_path(&path, &[]) {
                Ok(Some(ResolvedTypeName::Annotation(id))) => id,
                Ok(Some(_)) => {
                    self.error(
                        annotation.name.span,
                        format!("`{}` is not an annotation class", annotation.name),
                    );
                    continue;
                }
                Ok(None) => {
                    self.error(
                        annotation.name.span,
                        format!("unknown annotation `{}`", annotation.name),
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
                        annotation.name
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

    pub(super) fn bind_annotation_arguments(
        &mut self,
        annotation: &ast::Annotation,
        parameters: &[hir::SourceAnnotationParameter],
    ) -> Option<Vec<hir::CanonicalAnnotationValueV1>> {
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
            values[index] =
                self.annotation_value(&argument.value, parameters[index].value_type, argument.span);
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
