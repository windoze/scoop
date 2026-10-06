//! Source roots for declared fields and object property storage.

use super::*;
use scoop_identity::{
    EnumVariantFieldKey, FieldIdentityKey, FieldIdentityView, GeneratedNominalKey,
    PersistentEnumVariantFieldId, PersistentFieldId, SourceDeclarationKind,
};

impl CanonicalCrossConeRouteAuthority<'_> {
    pub(super) fn field_resolution(
        &self,
        field: PersistentFieldId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentFieldId, FieldIdentityKey>(field)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
        let owner = match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
            FieldIdentityView::Generated { owner, key } => {
                let property = key
                    .object_backing_property()
                    .ok_or(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target })?;
                self.object_field_owner(field, owner, property)?
            }
        };
        self.nominal_resolution(owner)
    }

    fn object_field_owner(
        &self,
        field: PersistentFieldId,
        generated: PersistentTypeId,
        property: PersistentPropertyId,
    ) -> Result<NominalDeclarationOwner, CrossConeHirReferenceAuthorityError> {
        let target = ExternalHirTargetV1::Field(field);
        let key = self
            .identities
            .canonical_key::<PersistentTypeId, GeneratedNominalKey>(generated)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
        let (object, atom) = match key.as_ref() {
            GeneratedNominalKey::ObjectBackingClass { object } => (
                NominalDeclarationOwner::Concrete(*object),
                DefinitionOwnerAtom::Type(*object),
            ),
            GeneratedNominalKey::GenericObjectBackingClass { object } => (
                NominalDeclarationOwner::GenericTemplate(*object),
                DefinitionOwnerAtom::GenericType(*object),
            ),
            _ => return Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target }),
        };
        let object_key = self.nominal_source_key(object)?;
        if object_key.declaration_kind() != SourceDeclarationKind::Object {
            return Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target });
        }
        let property_key = self.source_declaration_key(property)?;
        if property_key.owners().owners().last() != Some(&atom)
            || property_key.origin() != object_key.origin()
        {
            return Err(
                CrossConeHirReferenceAuthorityError::ObjectFieldOwnerMismatch(Box::new(
                    ExternalObjectFieldOwnerMismatch { field, property },
                )),
            );
        }
        Ok(object)
    }

    pub(super) fn variant_field_resolution(
        &self,
        field: PersistentEnumVariantFieldId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentEnumVariantFieldId, EnumVariantFieldKey>(field)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
        self.variant_resolution(key.variant(), target)
    }
}
