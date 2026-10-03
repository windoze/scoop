use super::*;
use scoop_hir::{IntegerKind, IntrinsicTypeKind, NativeBoundaryCAbiV1};

impl NativeBoundaryNormalizer<'_> {
    pub(super) fn projected_c_storage(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<Option<CanonicalCStorageType>, NativeBoundaryCompileError> {
        if !matches!(
            self.exact(exact)?,
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. }
        ) {
            return Ok(None);
        }
        let (definition, binders) = self.definition(exact)?;
        let storage = match definition.c_abi() {
            NativeBoundaryCAbiV1::SourceRepresentation => return Ok(None),
            NativeBoundaryCAbiV1::UInt64Field { field } => {
                let NativeBoundaryNominalShape::Struct { fields, .. } = definition.shape() else {
                    return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
                };
                let [source] = fields.as_slice() else {
                    return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
                };
                if source.field() != field {
                    return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
                }
                let field_exact = self.signature_exact(source.ty(), &binders)?;
                if self.intrinsic(field_exact)?
                    != Some(IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_64))
                {
                    return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
                }
                CanonicalCStorageType::Integer {
                    exact_type: exact,
                    signedness: scoop_identity::Signedness::Unsigned,
                    bit_width: scoop_identity::IntegerBitWidth::Bits64,
                }
            }
            NativeBoundaryCAbiV1::NullablePointer { payload, .. } => {
                let NativeBoundaryNominalShape::Enum { variants } = definition.shape() else {
                    return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
                };
                let source = variants
                    .iter()
                    .flat_map(|variant| variant.fields())
                    .find(|field| field.field() == payload)
                    .ok_or(NativeBoundaryTargetError::InvalidSignatureShape)?;
                let payload = self.signature_exact(source.ty(), &binders)?;
                if !matches!(
                    self.exact(payload)?,
                    ExactTypeKey::RawPointer(_) | ExactTypeKey::NativeFunctionPointer { .. }
                ) {
                    return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
                }
                match self.c_storage(payload)? {
                    CanonicalCStorageType::DataPointer {
                        pointee,
                        storage: CPointerStorage::Direct,
                        ..
                    } => CanonicalCStorageType::DataPointer {
                        exact_type: exact,
                        pointee,
                        storage: CPointerStorage::NullableWrapper(exact),
                    },
                    CanonicalCStorageType::CodePointer {
                        storage: CPointerStorage::Direct,
                        ..
                    } => CanonicalCStorageType::CodePointer {
                        exact_type: exact,
                        storage: CPointerStorage::NullableWrapper(exact),
                    },
                    _ => return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into()),
                }
            }
        };
        Ok(Some(storage))
    }
}
