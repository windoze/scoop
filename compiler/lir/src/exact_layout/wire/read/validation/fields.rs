use super::*;

pub(super) fn nominal_fields(
    raw: Vec<RawNominalField>,
    expected: &[crate::PlacedFieldStorageV1],
) -> Result<(), ExactLayoutWireError> {
    table_length(raw.len(), expected.len())?;
    for (raw, expected) in raw.into_iter().zip(expected) {
        verify(raw.id, expected.field())?;
        if raw.alignment != expected.access_alignment().get() {
            return Err(ExactLayoutWireError::FieldMismatch);
        }
        raw.storage.validate_against(expected.storage())?;
    }
    Ok(())
}

impl RawVariant {
    pub(super) fn validate_against(
        self,
        expected: &EnumVariantLayoutV1,
    ) -> Result<(), ExactLayoutWireError> {
        verify(self.id, expected.variant())?;
        table_length(self.fields.len(), expected.fields().len())?;
        for (raw, expected) in self.fields.into_iter().zip(expected.fields()) {
            verify(raw.id, expected.field())?;
            if raw.alignment != expected.access_alignment().get() {
                return Err(ExactLayoutWireError::FieldMismatch);
            }
            raw.storage.validate_against(expected.storage())?;
        }
        Ok(())
    }
}

impl RawRegion {
    pub(super) fn validate_against(
        &self,
        expected: crate::EnumStorageRegionV1,
    ) -> Result<(), ExactLayoutWireError> {
        if self.offset == expected.offset()
            && self.size == expected.byte_size()
            && self.alignment == expected.alignment().get()
        {
            Ok(())
        } else {
            Err(ExactLayoutWireError::RegionMismatch)
        }
    }
}

impl RawSlot {
    pub(super) fn validate_against(
        &self,
        expected: &crate::EnumVariantStorageGeometryV1,
    ) -> Result<(), ExactLayoutWireError> {
        match (self, expected.slot()) {
            (Self::Shared { size, alignment }, crate::EnumVariantSlotV1::SharedPure(_))
                if *size == expected.storage().size()
                    && *alignment == expected.storage().alignment().get() =>
            {
                Ok(())
            }
            (Self::Dedicated(raw), crate::EnumVariantSlotV1::Dedicated(expected)) => {
                raw.validate_against(expected)
            }
            _ => Err(ExactLayoutWireError::RegionMismatch),
        }
    }
}
