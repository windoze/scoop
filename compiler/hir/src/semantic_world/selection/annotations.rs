use super::ImportedDependencySelectionPlan;
use crate::{AnnotationApplicationV1, AnnotationDeclarationV1, AnnotationTargetV1};
use scoop_identity::{PersistentAnnotationId, SourceDeclarationKey};

#[derive(Clone, Debug)]
pub struct ImportedAnnotationDeclaration {
    pub source: SourceDeclarationKey,
    pub declaration: AnnotationDeclarationV1,
}

impl ImportedDependencySelectionPlan {
    pub fn annotation_value_types(
        &self,
    ) -> impl Iterator<Item = &scoop_identity::SignatureTypeKey> {
        self.catalog.annotations.values().flat_map(|annotation| {
            annotation
                .declaration
                .parameters
                .iter()
                .filter_map(|parameter| match &parameter.value_type {
                    crate::AnnotationParameterType::Value(ty) => Some(ty),
                    _ => None,
                })
        })
    }

    pub fn annotated_type(
        &self,
        target: AnnotationTargetV1,
        slot: crate::AnnotationTypeSlot,
    ) -> Option<&crate::AnnotatedTypeUseV1> {
        self.catalog.annotated_types.get(&(target, slot))
    }

    pub fn annotation_declaration(
        &self,
        id: PersistentAnnotationId,
    ) -> Option<&ImportedAnnotationDeclaration> {
        self.catalog.annotations.get(&id)
    }

    pub fn annotations(&self, target: AnnotationTargetV1) -> &[AnnotationApplicationV1] {
        self.catalog
            .annotated_targets
            .get(&target)
            .map_or(&[], Vec::as_slice)
    }
}
