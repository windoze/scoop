use super::*;
use crate::types::ResolvedTypeName;
use std::collections::HashSet;

impl Lowerer {
    pub(crate) fn bind_source_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[ast::Annotation],
    ) {
        self.bind_annotations(target, annotations, true);
    }

    pub(super) fn bind_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[ast::Annotation],
        check_rules: bool,
    ) {
        for annotation in annotations {
            let Some(target) = self.annotation_use_site(target, annotation) else {
                continue;
            };
            if annotation
                .name
                .single_text()
                .is_some_and(crate::annotations::is_core_annotation)
            {
                self.check_core_annotation_target(target, annotation);
                continue;
            }
            let Some(id) = self.resolve_user_annotation(annotation) else {
                continue;
            };
            let repeated = self
                .annotation_metadata
                .targets
                .iter()
                .find(|entry| entry.target == target)
                .is_some_and(|entry| entry.annotations.iter().any(|a| a.annotation == id));
            if check_rules
                && !self.check_annotation_rules(
                    id,
                    target.kind(),
                    repeated,
                    &annotation.name.to_string(),
                    annotation.span,
                )
            {
                continue;
            }
            if let Some(application) = self.bind_annotation_application(annotation, id) {
                self.validate_test_annotation_source(&application);
                self.validate_serialization_annotations(target, std::slice::from_ref(&application));
                if let Some(entry) = self
                    .annotation_metadata
                    .targets
                    .iter_mut()
                    .find(|entry| entry.target == target)
                {
                    entry.annotations.push(application);
                } else {
                    self.annotation_metadata
                        .targets
                        .push(hir::SourceAnnotatedTarget {
                            target,
                            annotations: vec![application],
                        });
                }
            }
        }
    }

    pub(super) fn resolve_user_annotation(
        &mut self,
        annotation: &ast::Annotation,
    ) -> Option<PersistentAnnotationId> {
        let path = annotation.name.segments().cloned().collect::<Vec<_>>();
        match self.resolve_type_name_path(&path, &[]) {
            Ok(Some(ResolvedTypeName::Annotation(id))) => Some(id),
            Ok(Some(_)) => {
                self.error(
                    annotation.name.span,
                    format!("`{}` is not an annotation class", annotation.name),
                );
                None
            }
            Ok(None) => {
                self.error(
                    annotation.name.span,
                    format!("unknown annotation `{}`", annotation.name),
                );
                None
            }
            Err(()) => None,
        }
    }

    pub(super) fn bind_annotation_application(
        &mut self,
        annotation: &ast::Annotation,
        id: PersistentAnnotationId,
    ) -> Option<hir::SourceAnnotationApplication> {
        let parameters = self.annotation_parameters(id, annotation.span)?;
        let arguments = self.bind_annotation_arguments(annotation, &parameters)?;
        Some(hir::SourceAnnotationApplication {
            annotation: id,
            arguments,
            definition_origin: self.definition_origin(annotation.span),
        })
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
