use super::*;
use scoop_identity::{PersistentAnnotationId, SourceDeclarationKey};

#[derive(Clone, Copy)]
pub enum StaticAnnotationDeclaration<'a> {
    Current(&'a SourceAnnotationDeclaration),
    Dependency {
        key: &'a SourceDeclarationKey,
        declaration: &'a AnnotationDeclarationV1,
        annotations: &'a CanonicalAnnotationsV1,
    },
}

impl<'a> Module {
    pub fn static_annotation_declaration(
        &'a self,
        id: PersistentAnnotationId,
        world: &ImportedSemanticWorld<'a>,
    ) -> StaticAnnotationDeclaration<'a> {
        self.annotations
            .declarations
            .iter()
            .find(|declaration| declaration.identity.id() == id)
            .map(StaticAnnotationDeclaration::Current)
            .or_else(|| world.static_annotation_declaration(id))
            .expect("a bound annotation retains its declaration")
    }
}

impl<'a> StaticAnnotationDeclaration<'a> {
    pub fn identity(self) -> PersistentAnnotationId {
        match self {
            Self::Current(declaration) => declaration.identity.id(),
            Self::Dependency { declaration, .. } => declaration.annotation,
        }
    }

    pub fn key(self) -> &'a SourceDeclarationKey {
        match self {
            Self::Current(declaration) => declaration.identity.key(),
            Self::Dependency { key, .. } => key,
        }
    }

    pub fn visibility(self) -> DeclaredVisibilityV1 {
        match self {
            Self::Current(declaration) => declaration.visibility.into(),
            Self::Dependency { declaration, .. } => declaration.visibility,
        }
    }

    pub fn annotations(self, module: &'a Module) -> StaticAnnotations<'a> {
        match self {
            Self::Current(_) => StaticAnnotations::current(
                module,
                SourceAnnotationTarget::Annotation(self.identity()),
            ),
            Self::Dependency { annotations, .. } => StaticAnnotations::published(
                annotations,
                AnnotationTargetV1::Annotation(self.identity()),
            ),
        }
    }
}
