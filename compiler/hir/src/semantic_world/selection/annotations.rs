use super::ImportedDependencySelectionPlan;
use crate::{AnnotationApplicationV1, AnnotationDeclarationV1, AnnotationTargetV1};
use scoop_identity::{PersistentAnnotationId, SourceDeclarationKey};

#[derive(Clone, Debug)]
pub struct ImportedAnnotationDeclaration {
    pub source: SourceDeclarationKey,
    pub declaration: AnnotationDeclarationV1,
}

impl ImportedDependencySelectionPlan {
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
