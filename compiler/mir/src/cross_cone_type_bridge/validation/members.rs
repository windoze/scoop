use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey, FieldIdentityView,
};

impl MirTypeBridgeAuthority<'_> {
    pub(super) fn validate_members(
        &self,
        record: &ParamFreeMirTypeExportV1,
    ) -> Result<(), MirTypeBridgeError> {
        let owner = record.origin().owner();
        if let MirTypeRepresentationV1::InlineArray { element }
        | MirTypeRepresentationV1::AtomicReference { value: element } = record.representation()
        {
            let key = self
                .identities
                .canonical_key::<_, ExactTypeKey>(record.exact())?;
            if !matches!(key.as_ref(), ExactTypeKey::NominalApplication { arguments, .. } if arguments.as_slice() == [*element])
            {
                return Err(MirTypeBridgeError::OriginRepresentationMismatch {
                    exact: record.exact(),
                });
            }
            self.identities.canonical_key::<_, ExactTypeKey>(*element)?;
        }
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
                    MirTypeOriginV1::SourceNominal(_) | MirTypeOriginV1::NominalApplication(_),
                    FieldIdentityView::SourceDeclared { owner: actual, .. }
                    | FieldIdentityView::SourcePropertyBacking { owner: actual, .. }
                    | FieldIdentityView::SourcePropertyDelegate { owner: actual, .. },
                ) => actual == owner,
                (
                    MirTypeOriginV1::GeneratedNominal { nominal, .. },
                    FieldIdentityView::Generated { owner: actual, .. },
                ) => actual == *nominal,
                (
                    MirTypeOriginV1::NominalApplication(object),
                    FieldIdentityView::Generated { owner: actual, .. },
                ) => {
                    let key = GeneratedNominalKey::GenericObjectBackingClass { object: *object };
                    self.identities
                        .canonical_key::<_, SourceDeclarationKey>(*object)?
                        .declaration_kind()
                        == SourceDeclarationKind::Object
                        && PersistentTypeId::from_generated_key(&key).ok() == Some(actual)
                }
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
            if matches!(
                record.origin(),
                MirTypeOriginV1::SourceNominal(_) | MirTypeOriginV1::NominalApplication(_)
            ) && key.source_owner() != Some(owner)
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
