use super::*;
use scoop_identity::{FieldIdentityKey, FieldIdentityView, SignatureTypeKey};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn constructor_outer_receiver(
        &self,
        owner: PublicDeclarationOwnerV1,
    ) -> Result<Option<SignatureTypeKey>, CrossConeHirNominalAuthorityError> {
        let PublicDeclarationOwnerV1::Nominal(owner) = owner else {
            return Ok(None);
        };
        let record = self
            .current_interface
            .nominal_interfaces()
            .declaration(owner)
            .ok_or(CrossConeHirNominalAuthorityError::MissingNominalInterface {
                origin: self.current,
                declaration: owner,
            })?;
        for field in record.source_shape().declared_fields() {
            let key = self
                .identities
                .canonical_key::<_, FieldIdentityKey>(field.field())
                .map_err(CrossConeHirNominalAuthorityError::Identity)?;
            if matches!(
                key.view(),
                FieldIdentityView::SourceEnclosingInstance { .. }
            ) {
                return Ok(Some(field.value_type().clone()));
            }
        }
        Ok(None)
    }
}
