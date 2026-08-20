use scoop_hir::{
    ExportBindingSourceV1, PublicLookupAccessV1, TypeAliasDeclarationSourceV1,
    TypeAliasInterfaceSemanticAuthority,
};
use scoop_identity::{
    BindableEntity, DefinitionOriginSubject, PersistentTypeAliasId, SourceDeclarationKey,
};

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError};

impl TypeAliasInterfaceSemanticAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> scoop_identity::ConeIdentity {
        self.current
    }

    fn type_alias_declaration_source(
        &mut self,
        alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, CrossConeHirNominalAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentTypeAliasId, SourceDeclarationKey>(alias)
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        self.require_current("type-alias interface", key.origin())?;

        let is_direct_public = self
            .current_interface
            .public_bindings()
            .records()
            .iter()
            .any(|record| {
                matches!(
                    record.source(),
                    ExportBindingSourceV1::DeclaredCurrent {
                        declaration: BindableEntity::TypeAlias(bound)
                    } if *bound == alias
                )
            });
        if !is_direct_public {
            return Err(
                CrossConeHirNominalAuthorityError::MissingDirectPublicTypeAliasBinding { alias },
            );
        }

        let subject = DefinitionOriginSubject::TypeAlias(alias);
        let origin = self
            .current_foundation
            .definition_origin(subject)
            .ok_or(CrossConeHirNominalAuthorityError::MissingDefinitionOrigin { subject })?;
        Ok(TypeAliasDeclarationSourceV1::new(
            key.as_ref().clone(),
            PublicLookupAccessV1::DirectOnly,
            origin.origin().clone(),
        ))
    }
}
