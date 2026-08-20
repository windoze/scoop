use super::*;
use hir::{AnnotationRules, AnnotationTargetKind};

mod declarations;

impl Lowerer {
    pub(super) fn initial_annotation_rules(&self, id: PersistentAnnotationId) -> AnnotationRules {
        if self.core_source_annotation("Target") == Some(id)
            || self.core_source_annotation("Repeatable") == Some(id)
        {
            AnnotationRules::only(AnnotationTargetKind::AnnotationClass)
        } else {
            AnnotationRules::default()
        }
    }

    pub(super) fn annotation_rules(&self, id: PersistentAnnotationId) -> Option<AnnotationRules> {
        self.annotation_metadata
            .declarations
            .iter()
            .find(|declaration| declaration.identity.id() == id)
            .map(|declaration| declaration.rules)
            .or_else(|| {
                self.dependencies
                    .as_ref()?
                    .annotation_declaration(id)
                    .map(|declaration| declaration.declaration.rules)
            })
    }

    pub(super) fn check_annotation_rules(
        &mut self,
        id: PersistentAnnotationId,
        target: AnnotationTargetKind,
        repeated: bool,
        name: &str,
        span: ast::Span,
    ) -> bool {
        let Some(rules) = self.annotation_rules(id) else {
            return false;
        };
        if !rules.permits(target) {
            self.error(
                span,
                format!("annotation `@{name}` is not allowed on {}", target.name()),
            );
            return false;
        }
        if repeated && !rules.repeatable {
            self.error(
                span,
                format!("annotation `@{name}` must not be repeated on the same target"),
            );
            return false;
        }
        true
    }

    pub(super) fn annotation_name(&self, id: PersistentAnnotationId) -> String {
        let key = self
            .source_annotations
            .get(&id)
            .map(|input| input.identity.key())
            .or_else(|| {
                self.dependencies
                    .as_ref()?
                    .annotation_declaration(id)
                    .map(|declaration| &declaration.source)
            });
        match key.map(|key| key.name()) {
            Some(scoop_identity::DeclarationName::Named(name)) => name.as_str().into(),
            _ => id.to_string(),
        }
    }
}
