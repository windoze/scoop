//! Exact source roots for ordinary and object-backed field references.

use super::*;
use scoop_identity::{
    FieldIdentityView, GeneratedNominalKey, PersistentFieldId, SourceDeclarationKind,
};

impl CrossConeHirProductionAuthority<'_, '_> {
    pub(super) fn field_resolution(
        &self,
        field: PersistentFieldId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let missing = || CrossConeHirProductionAuthorityError::MissingCanonicalKey { target };
        let no_root = || CrossConeHirProductionAuthorityError::NoPublicBindingRoot { target };
        let key = self.field_key(field).ok_or_else(missing)?;
        let owner = match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
            FieldIdentityView::Generated { owner, key } => {
                let property = key.object_backing_property().ok_or_else(no_root)?;
                let generated = self.generated_type_key(owner).ok_or_else(missing)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = generated else {
                    return Err(no_root());
                };
                let object_key = self.source_type_key(*object).ok_or_else(missing)?;
                if object_key.declaration_kind() != SourceDeclarationKind::Object {
                    return Err(no_root());
                }
                let property_key = self.property_key(property).ok_or_else(missing)?;
                if property_key.owners().owners().last()
                    != Some(&DefinitionOwnerAtom::Type(*object))
                    || property_key.origin() != object_key.origin()
                {
                    return Err(
                        CrossConeHirProductionAuthorityError::InvalidObjectFieldOwner(field),
                    );
                }
                NominalDeclarationOwner::Concrete(*object)
            }
        };
        self.nominal_resolution(owner, target)
    }
}
