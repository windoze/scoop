use super::*;

mod c_projection;
mod intrinsics;
mod scoop;
use intrinsics::{integer_representation, intrinsic_layout, is_reference};

impl<'a> NativeBoundaryNormalizer<'a> {
    pub(super) fn c_storage(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<CanonicalCStorageType, NativeBoundaryCompileError> {
        enum Shape {
            DataPointer(PersistentExactTypeId),
            CodePointer,
            CLayout,
            Unsupported,
        }
        if self.is_unit(exact) {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        }
        if let Some(family) = self.intrinsic(exact)? {
            return match family {
                scoop_hir::IntrinsicTypeKind::Integer(kind) => {
                    let (signedness, bit_width) = integer_representation(kind);
                    Ok(CanonicalCStorageType::Integer {
                        exact_type: exact,
                        signedness,
                        bit_width,
                    })
                }
                scoop_hir::IntrinsicTypeKind::Float(kind) => Ok(CanonicalCStorageType::Float {
                    exact_type: exact,
                    kind,
                }),
                scoop_hir::IntrinsicTypeKind::Char => Ok(CanonicalCStorageType::Integer {
                    exact_type: exact,
                    signedness: scoop_identity::Signedness::Unsigned,
                    bit_width: scoop_identity::IntegerBitWidth::Bits32,
                }),
                scoop_hir::IntrinsicTypeKind::Boolean => {
                    Ok(CanonicalCStorageType::Boolean { exact_type: exact })
                }
                scoop_hir::IntrinsicTypeKind::Ptr | scoop_hir::IntrinsicTypeKind::FunPtr => {
                    Err(NativeBoundaryTargetError::InvalidSignatureShape.into())
                }
                scoop_hir::IntrinsicTypeKind::Unit
                | scoop_hir::IntrinsicTypeKind::Atomic(_)
                | scoop_hir::IntrinsicTypeKind::Any
                | scoop_hir::IntrinsicTypeKind::Nothing
                | scoop_hir::IntrinsicTypeKind::String
                | scoop_hir::IntrinsicTypeKind::Array
                | scoop_hir::IntrinsicTypeKind::MutableArray => {
                    Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into())
                }
            };
        }
        if let Some(storage) = self.projected_c_storage(exact)? {
            return Ok(storage);
        }
        let key = self
            .exact_types
            .get(&exact)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact })?;
        let shape = match key.as_ref() {
            ExactTypeKey::RawPointer(pointee) => Shape::DataPointer(*pointee),
            ExactTypeKey::NativeFunctionPointer { .. } => Shape::CodePointer,
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => Shape::CLayout,
            ExactTypeKey::Tuple(_) | ExactTypeKey::Function { .. } => Shape::Unsupported,
        };
        match shape {
            Shape::DataPointer(pointee) => Ok(CanonicalCStorageType::DataPointer {
                exact_type: exact,
                pointee: if self.is_unit(pointee) {
                    CDataPointee::OpaqueUnit
                } else {
                    CDataPointee::ExactObject(pointee)
                },
                storage: CPointerStorage::Direct,
            }),
            Shape::CodePointer => Ok(CanonicalCStorageType::CodePointer {
                exact_type: exact,
                storage: CPointerStorage::Direct,
            }),
            Shape::CLayout => {
                let layout = self.c_layout(exact)?;
                Ok(CanonicalCStorageType::Struct {
                    exact_type: exact,
                    layout,
                })
            }
            Shape::Unsupported => Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into()),
        }
    }

    fn c_layout(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<CanonicalCAbiLayoutFingerprint, NativeBoundaryCompileError> {
        if let Some(fingerprint) = self.layouts_by_type.get(&exact) {
            return Ok(*fingerprint);
        }
        if !self
            .visiting_c_layouts
            .push(exact, &WirePath::root().field(16))?
        {
            return Err(NativeBoundaryTargetError::CLayoutCycle { exact }.into());
        }
        let (definition, binders) = self.definition(exact)?;
        let NativeBoundaryNominalShape::Struct { c_layout, fields } = definition.shape() else {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        };
        let NativeBoundaryCLayoutPolicy::CLayout { aligned, packed } = c_layout else {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        };
        let aligned = *aligned;
        let packed = *packed;
        let mut normalized = allocate_vec(fields.len(), &WirePath::root().field(16))?;
        let mut size = 0_u64;
        let mut alignment = override_bytes(aligned).unwrap_or(1);
        for field in fields {
            let field_exact = self.signature_exact(field.ty(), &binders)?;
            let storage = self.c_storage(field_exact)?;
            let (field_size, natural_alignment) = self.c_storage_layout(storage)?;
            let access_alignment = override_bytes(packed)
                .map_or(natural_alignment, |packed| natural_alignment.min(packed));
            let offset = align_up(size, access_alignment)?;
            size = offset
                .checked_add(field_size)
                .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
            alignment = alignment.max(access_alignment);
            normalized.push(CanonicalCAbiLayoutField::new(
                field.field(),
                offset,
                storage,
            ));
        }
        size = align_up(size, alignment)?;
        let alignment = NonZeroU64::new(alignment)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        let layout = CanonicalCAbiLayout::new(exact, size, alignment, aligned, packed, normalized);

        let record = CanonicalCAbiLayoutFingerprintRecord::new(layout)
            .map_err(NativeBoundaryTargetError::Hash)?;
        let fingerprint = record.fingerprint();
        insert_entry(
            &mut self.expected_layouts,
            fingerprint,
            record,
            &WirePath::root().field(16),
        )?;
        insert_entry(
            &mut self.layouts_by_type,
            exact,
            fingerprint,
            &WirePath::root().field(16),
        )?;
        self.visiting_c_layouts.remove(&exact);
        Ok(fingerprint)
    }

    fn c_storage_layout(
        &mut self,
        storage: CanonicalCStorageType,
    ) -> Result<(u64, u64), NativeBoundaryCompileError> {
        match storage {
            CanonicalCStorageType::Integer { bit_width, .. } => {
                let layout = self.target.scalar_layout(integer_scalar_kind(bit_width));
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::Float { kind, .. } => {
                let layout = self.target.float_layout(kind);
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::Boolean { .. } => {
                let layout = self.target.scalar_layout(scoop_lir::BackendScalarKind::I1);
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::DataPointer { .. } => {
                let layout = self.target.data_pointer().layout();
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::CodePointer { .. } => {
                let layout = self.target.code_pointer().layout();
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::Struct { layout, .. } => {
                let record = self
                    .expected_layouts
                    .get(&layout)
                    .ok_or(NativeBoundaryTargetError::MissingComputedCLayout { layout })?;
                Ok((
                    record.layout().byte_size(),
                    record.layout().alignment().get(),
                ))
            }
        }
    }

    fn definition(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<
        (
            &'a AbiNominalDefinition<'a>,
            Vec<Vec<PersistentExactTypeId>>,
        ),
        NativeBoundaryCompileError,
    > {
        let path = WirePath::root().field(1);

        let mut binders = Vec::new();
        let key = self
            .exact_types
            .get(&exact)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact })?;
        let owner = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => NativeBoundaryNominalOwner::Concrete(*owner),
            ExactTypeKey::NominalApplication { origin, arguments } => {
                let owner = NativeBoundaryNominalOwner::GenericTemplate(*origin);
                push_binder_group(&mut binders, arguments.as_slice(), &path)?;
                owner
            }
            _ => return Err(NativeBoundaryTargetError::ExpectedNominal { exact }.into()),
        };

        self.definitions
            .get(&owner)
            .map(|definition| (definition, binders))
            .ok_or(NativeBoundaryCompileError::ClosureRequired { owner })
    }

    pub(super) fn exact(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, NativeBoundaryCompileError> {
        self.exact_types
            .get(&exact)
            .map(Arc::as_ref)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact }.into())
    }

    pub(super) fn is_unit(&self, exact: PersistentExactTypeId) -> bool {
        matches!(
            self.exact_types.get(&exact).map(Arc::as_ref),
            Some(ExactTypeKey::Nominal(owner))
                if *owner == scoop_identity::CoreBuiltinNominal::Unit.identity_record().id()
        )
    }
}

#[cfg(test)]
mod tests;
