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

impl Projection<'_, '_> {
    pub(super) fn enumeration(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &[mir::MirRepresentationVariantV1],
        depth: u64,
    ) -> Result<lir::ExactValueLayoutV1> {
        let owner = identity.exact();
        let mut variants = self.reserve(source.len())?;
        for variant in source {
            self.meter.charge_work(1, &WirePath::root())?;
            let identity = self.identities.canonical_record(variant.variant)?;
            let mut fields = self.reserve(variant.fields.len())?;
            let mut values = self.reserve(variant.fields.len())?;
            for field in &variant.fields {
                self.meter.charge_work(1, &WirePath::root())?;
                fields.push(self.identities.canonical_record(field.field)?);
                values.push(self.value_dependency(field.value, depth)?);
            }
            let has_references = values.iter().any(|value| {
                value.value().storage().nonzero().is_some_and(|storage| {
                    !matches!(storage.scan().as_ref_scan(), lir::RefScan::None)
                })
            });
            if has_references != (variant.gc == mir::MirGcKindV1::ContainsManagedReferences) {
                return Err(ExactLayoutLoweringError::SourceFacts(owner));
            }
            variants.push(Variant {
                identity,
                fields,
                values,
            });
        }
        let mut fields = self.reserve(variants.len())?;
        for variant in &variants {
            let mut input = self.reserve(variant.fields.len())?;
            input.extend(
                variant
                    .fields
                    .iter()
                    .zip(&variant.values)
                    .map(|(field, value)| lir::EnumLayoutFieldInputV1 { field, value }),
            );
            fields.push(input);
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
            self.output.foundation(),
            self.meter,
        )?)
    }
}
