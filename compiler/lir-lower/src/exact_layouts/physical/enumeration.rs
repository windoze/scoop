use super::*;

impl Projection<'_> {
    pub(super) fn validate_enum(
        &mut self,
        value: &lir::ExactValueLayoutV1,
        id: mir::EnumId,
    ) -> Result<()> {
        let error = || ExactLayoutLoweringError::PhysicalLayout(value.identity().layout());
        let actual = self
            .output
            .module()
            .enums
            .get(crate::metadata::enum_def_id(id))
            .ok_or_else(error)?;
        match (value.representation().kind(), &actual.repr) {
            (
                lir::ExactRepresentationKindV1::NicheEnum(expected),
                lir::EnumRepr::Niche {
                    kind,
                    payload_variant,
                },
            ) => {
                if expected.pointer_kind() != *kind
                    || expected
                        .variants()
                        .get(*payload_variant as usize)
                        .is_none_or(|variant| variant.variant() != expected.payload_variant())
                {
                    return Err(error());
                }
            }
            (
                lir::ExactRepresentationKindV1::TaggedEnum(expected),
                lir::EnumRepr::Tagged {
                    variants,
                    size,
                    align,
                },
            ) => {
                if expected.geometry().storage().size() != *size
                    || expected.geometry().storage().alignment().get() != *align
                    || expected.variants().len() != variants.len()
                {
                    return Err(error());
                }

                for (((variant, geometry), actual), source) in expected
                    .variants()
                    .iter()
                    .zip(expected.geometry().variants())
                    .zip(variants)
                    .zip(&self.module.enums[id].variants)
                {
                    if actual.slot_offset != geometry.slot().region().offset()
                        || actual.slot_size != geometry.storage().size()
                        || actual.slot_align != geometry.storage().alignment().get()
                        || actual.fields.len() != variant.fields().len()
                        || actual.gc_free
                            != matches!(geometry.slot(), lir::EnumVariantSlotV1::SharedPure(_))
                    {
                        return Err(error());
                    }

                    for ((field, actual), source) in variant
                        .fields()
                        .iter()
                        .zip(&actual.fields)
                        .zip(&source.fields)
                    {
                        if field.storage().offset().get() != actual.offset
                            || actual.ty != crate::metadata::lir_type(self.module, &source.ty)
                        {
                            return Err(error());
                        }
                    }
                }
            }
            _ => return Err(error()),
        }
        let scan = match value.value().storage().kind() {
            lir::ValueStorageKindV1::ZeroSized { .. } => &lir::RefScan::None,
            lir::ValueStorageKindV1::Inline { scan, .. } => scan.as_ref_scan(),
        };
        if !super::scan::matches(scan, &actual.scan)? {
            return Err(error());
        }
        Ok(())
    }
}
