use scoop_identity::{
    CLayoutOverride, CPointerStorage, CanonicalCAbiLayoutFingerprintRecord, CanonicalCStorageType,
    IntegerBitWidth,
};

use super::*;

/// Replay of an already selected canonical C storage contract. This does not
/// classify source types as C-FFI-safe or infer any C calling convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CLayoutStorageReplayV1 {
    target: LirTargetProfile,
    contract: CanonicalCAbiLayoutFingerprintRecord,
    aggregate: AggregateStorageLayoutV1,
}

impl CLayoutStorageReplayV1 {
    pub fn replay(
        target: LirTargetProfile,
        contract: &CanonicalCAbiLayoutFingerprintRecord,
        fields: &[DeclaredFieldStorageV1<'_>],
        nested: &[&Self],
    ) -> Result<Self, StorageReplayError> {
        let canonical = contract.layout();
        if fields.is_empty() {
            return Err(StorageReplayError::EmptyCLayout);
        }
        if canonical.fields().len() != fields.len() {
            return Err(StorageReplayError::CLayoutMismatch);
        }
        for (field, expected) in fields.iter().zip(canonical.fields()) {
            let Some(storage) = field.layout.storage.nonzero() else {
                return Err(StorageReplayError::InvalidCField(field.field));
            };
            let exact = match expected.storage() {
                CanonicalCStorageType::DataPointer {
                    storage: CPointerStorage::NullableWrapper(exact),
                    ..
                }
                | CanonicalCStorageType::CodePointer {
                    storage: CPointerStorage::NullableWrapper(exact),
                    ..
                } => exact,
                storage => storage.exact_type(),
            };
            if expected.field() != field.field
                || exact != field.layout.exact()
                || !matches!(storage.scan().as_ref_scan(), RefScan::None)
            {
                return Err(StorageReplayError::InvalidCField(field.field));
            }
            let (size, alignment) = c_storage_shape(target, expected.storage(), nested)?;
            if storage.size().get() != size || storage.alignment().get() != alignment {
                return Err(StorageReplayError::InvalidCField(field.field));
            }
        }
        let policy = crate::LirCLayoutContract {
            aligned: c_alignment(canonical.aligned()),
            packed: c_alignment(canonical.packed()),
        };
        let cursor = StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::CLayout(policy))
            .map_err(StorageReplayError::Shape)?;
        let placed = aggregate::place(target, fields, cursor, BTreeSet::new())?;
        let aggregate = aggregate::finish(target, placed)?;
        if aggregate.storage.byte_size() != canonical.byte_size()
            || aggregate.storage.alignment().get() != canonical.alignment().get()
            || aggregate
                .fields
                .iter()
                .zip(canonical.fields())
                .any(|(field, expected)| field.storage.offset().get() != expected.offset())
        {
            return Err(StorageReplayError::CLayoutMismatch);
        }
        Ok(Self {
            target,
            contract: contract.clone(),
            aggregate,
        })
    }

    pub const fn contract(&self) -> &CanonicalCAbiLayoutFingerprintRecord {
        &self.contract
    }
    pub const fn aggregate(&self) -> &AggregateStorageLayoutV1 {
        &self.aggregate
    }
}

fn c_alignment(value: CLayoutOverride) -> crate::LirCLayoutValue {
    use scoop_identity::CLayoutByteAlignment as A;
    match value {
        CLayoutOverride::Natural => crate::LirCLayoutValue::Natural,
        CLayoutOverride::Bytes(value) => match value {
            A::Bytes1 => crate::LirCLayoutValue::A1,
            A::Bytes2 => crate::LirCLayoutValue::A2,
            A::Bytes4 => crate::LirCLayoutValue::A4,
            A::Bytes8 => crate::LirCLayoutValue::A8,
            A::Bytes16 => crate::LirCLayoutValue::A16,
        },
    }
}

fn c_storage_shape(
    target: LirTargetProfile,
    storage: CanonicalCStorageType,
    nested: &[&CLayoutStorageReplayV1],
) -> Result<(u64, u64), StorageReplayError> {
    use crate::BackendScalarKind as Scalar;
    let layout = match storage {
        CanonicalCStorageType::Integer { bit_width, .. } => target.scalar_layout(match bit_width {
            IntegerBitWidth::Bits8 => Scalar::I8,
            IntegerBitWidth::Bits16 => Scalar::I16,
            IntegerBitWidth::Bits32 => Scalar::I32,
            IntegerBitWidth::Bits64 => Scalar::I64,
        }),
        CanonicalCStorageType::Boolean { .. } => target.scalar_layout(Scalar::I1),
        CanonicalCStorageType::DataPointer { .. } => target.data_pointer().layout(),
        CanonicalCStorageType::CodePointer { .. } => target.code_pointer().layout(),
        CanonicalCStorageType::Struct { exact_type, layout } => {
            let mut candidates = nested
                .iter()
                .copied()
                .filter(|entry| entry.contract.fingerprint() == layout);
            let entry = candidates
                .next()
                .ok_or(StorageReplayError::MissingNestedCLayout(layout))?;
            if candidates.next().is_some()
                || entry.target != target
                || entry.contract.layout().exact_type() != exact_type
            {
                return Err(StorageReplayError::CLayoutMismatch);
            }
            return Ok((
                entry.aggregate.storage.byte_size(),
                entry.aggregate.storage.alignment().get(),
            ));
        }
    };
    Ok((layout.size_bytes(), layout.alignment_bytes()))
}
