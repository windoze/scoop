use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey, FieldIdentityView,
    NominalDeclarationOwner,
};

impl MirTypeBridgeAuthority<'_> {
    pub(super) fn validate_members(
        &self,
        record: &ParamFreeMirTypeExportV1,
    ) -> Result<(), MirTypeBridgeError> {
        let owner = record.origin().nominal();
        let mut fields = std::collections::BTreeSet::new();
        for field in record.representation().fields() {
            if !fields.insert(field.field) {
                return Err(MirTypeBridgeError::DuplicateField { field: field.field });
            }
            let key = self
                .identities
                .canonical_key::<_, FieldIdentityKey>(field.field)?;
            let correct_owner = match (record.origin(), key.view()) {
                (
                    MirTypeOriginV1::SourceNominal(_),
                    FieldIdentityView::SourceDeclared { owner: actual, .. }
                    | FieldIdentityView::SourcePropertyBacking { owner: actual, .. }
                    | FieldIdentityView::SourcePropertyDelegate { owner: actual, .. },
                ) => actual == NominalDeclarationOwner::Concrete(owner),
                (
                    MirTypeOriginV1::GeneratedNominal { .. },
                    FieldIdentityView::Generated { owner: actual, .. },
                ) => actual == owner,
                _ => false,
            };
            if !correct_owner {
                return Err(MirTypeBridgeError::FieldOwner { field: field.field });
            }
            self.identities
                .canonical_key::<_, ExactTypeKey>(field.value)?;
        }
        let mut variants = std::collections::BTreeSet::new();
        let mut variant_fields = std::collections::BTreeSet::new();
        for variant in record.representation().variants() {
            if !variants.insert(variant.variant) {
                return Err(MirTypeBridgeError::DuplicateVariant {
                    variant: variant.variant,
                });
            }
            let key = self
                .identities
                .canonical_key::<_, EnumVariantIdentityKey>(variant.variant)?;
            if matches!(record.origin(), MirTypeOriginV1::SourceNominal(_))
                && key.source_owner() != Some(NominalDeclarationOwner::Concrete(owner))
            {
                return Err(MirTypeBridgeError::VariantOwner {
                    variant: variant.variant,
                });
            }
            for field in &variant.fields {
                if !variant_fields.insert(field.field) {
                    return Err(MirTypeBridgeError::DuplicateVariantField { field: field.field });
                }
                let key = self
                    .identities
                    .canonical_key::<_, EnumVariantFieldKey>(field.field)?;
                if key.variant() != variant.variant {
                    return Err(MirTypeBridgeError::VariantFieldOwner { field: field.field });
                }
                self.identities
                    .canonical_key::<_, ExactTypeKey>(field.value)?;
            }
        }
        Ok(())
    }
}
