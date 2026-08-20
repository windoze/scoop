use super::{
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKind, require_kind,
    require_non_generic,
};
use crate::{PersistentAnnotationId, ids::derive_persistent_id};

impl PersistentAnnotationId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::AnnotationClass,
            SourceDeclarationIdentityError::ExpectedAnnotation,
        )?;
        require_non_generic(key)?;
        derive_persistent_id("scoop-annotation-id-v1", key).map_err(Into::into)
    }
}
