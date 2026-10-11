use super::*;
use hir::{CanonicalAnnotationValueV1 as Value, SourceAnnotationTarget};
use std::collections::HashSet;

impl Lowerer {
    pub(in crate::user_annotations) fn bind_annotation_declaration_rules(
        &mut self,
        ids: &[PersistentAnnotationId],
    ) {
        for &id in ids {
            let input = &self.source_annotations[&id];
            self.current_file = input.file;
            self.current_owner = input.owner;
            let target = self.core_source_annotation("Target");
            let repeatable = self.core_source_annotation("Repeatable");
            let mut rules = self.initial_annotation_rules(id);
            for application in self.declaration_annotations(id) {
                if Some(application.annotation) == target {
                    let [Value::Array { elements, .. }] = application.arguments.as_slice() else {
                        self.error(
                            application.definition_origin.span,
                            "core Target requires an AnnotationTarget array".into(),
                        );
                        continue;
                    };
                    let mut allowed = AnnotationRules::none();
                    for element in elements {
                        let kind = match &element.value {
                            Value::IntegralEnum { value, .. } => {
                                AnnotationTargetKind::from_value(value.raw_bits())
                            }
                            _ => None,
                        };
                        let span = ast::Span::new(
                            u32::try_from(element.span.start_byte()).expect("source spans fit HIR"),
                            u32::try_from(element.span.end_byte()).expect("source spans fit HIR"),
                        );
                        match kind {
                            Some(kind) => {
                                if !allowed.insert(kind) {
                                    self.error(
                                        span,
                                        format!("duplicate annotation target `{}`", kind.name()),
                                    );
                                }
                            }
                            None => self.error(span, "unknown annotation target".into()),
                        }
                    }
                    allowed.repeatable = rules.repeatable;
                    rules = allowed;
                } else if Some(application.annotation) == repeatable {
                    rules.repeatable = true;
                }
            }
            if let Some(declaration) = self
                .annotation_metadata
                .declarations
                .iter_mut()
                .find(|declaration| declaration.identity.id() == id)
            {
                declaration.rules = rules;
            }
        }
        // All declarations now have their final rules, including forward references.
        for &id in ids {
            let input = &self.source_annotations[&id];
            self.current_file = input.file;
            self.current_owner = input.owner;
            let mut seen = HashSet::new();
            for application in self.declaration_annotations(id) {
                let name = self.annotation_name(application.annotation);
                self.check_annotation_rules(
                    application.annotation,
                    AnnotationTargetKind::AnnotationClass,
                    !seen.insert(application.annotation),
                    &name,
                    application.definition_origin.span,
                );
            }
        }
    }

    fn declaration_annotations(
        &self,
        id: PersistentAnnotationId,
    ) -> Vec<hir::SourceAnnotationApplication> {
        self.annotation_metadata
            .targets
            .iter()
            .find(|target| target.target == SourceAnnotationTarget::Annotation(id))
            .map_or_else(Vec::new, |target| target.annotations.clone())
    }
}
