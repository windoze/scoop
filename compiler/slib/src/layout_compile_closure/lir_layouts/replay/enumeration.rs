use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId,
};

use super::*;

struct Variant {
    identity: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
    fields: Vec<CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>>,
    values: Vec<Arc<lir::ExactValueLayoutV1>>,
}

impl Replay<'_> {
    pub(super) fn enumeration(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &[mir::MirRepresentationVariantV1],
    ) -> Result<lir::ExactValueLayoutV1> {
        let mut variants = self.reserve(source.len())?;
        for variant in source {
            let mut fields = self.reserve(variant.fields.len())?;
            let mut values = self.reserve(variant.fields.len())?;
            for field in &variant.fields {
                fields.push(self.identities.canonical_record(field.field)?);
                values.push(self.value_dependency(field.value)?);
            }
            let managed = values.iter().any(|value| {
                value.value().storage().nonzero().is_some_and(|storage| {
                    !matches!(storage.scan().as_ref_scan(), lir::RefScan::None)
                })
            });
            if managed != (variant.gc == mir::MirGcKindV1::ContainsManagedReferences) {
                return Err(Error::SourceFacts(identity.exact()));
            }
            variants.push(Variant {
                identity: self.identities.canonical_record(variant.variant)?,
                fields,
                values,
            });
        }
        let mut fields = self.reserve(variants.len())?;
        for variant in &variants {
            let mut inputs = self.reserve(variant.fields.len())?;
            inputs.extend(
                variant
                    .fields
                    .iter()
                    .zip(&variant.values)
                    .map(|(field, value)| lir::EnumLayoutFieldInputV1 { field, value }),
            );
            fields.push(inputs);
        }
        let mut inputs = self.reserve(variants.len())?;
        inputs.extend(variants.iter().zip(&fields).map(|(variant, fields)| {
            lir::EnumLayoutVariantInputV1 {
                variant: &variant.identity,
                fields,
            }
        }));
        Ok(lir::ExactValueLayoutV1::enumeration(
            identity,
            &inputs,
            self.foundation,
        )?)
    }
}
