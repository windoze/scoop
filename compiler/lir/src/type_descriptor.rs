use std::num::NonZeroU64;

use crate::{
    ArrayElementStorageKindV1, ArrayElementStorageV1, CheckedRefScanV1, LirTargetProfile,
    PointerKind, RefScan, RefScanValidationError, ValueStorageKindV1, ValueStorageLayoutV1,
};

/// Typed source of the runtime inline-scan pointer stored in one
/// `ScoopTypeDescriptor`.
///
/// The null and definition branches are explicit so codegen never has to
/// recover a scan identity from equal payload bytes or arena order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeDescriptorInlineScanV1 {
    Null,
    Defined(scoop_identity::PersistentScanId),
}

impl TypeDescriptorInlineScanV1 {
    pub const fn definition(self) -> Option<scoop_identity::PersistentScanId> {
        match self {
            Self::Null => None,
            Self::Defined(scan) => Some(scan),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeInstanceKindV1 {
    FixedObject,
    BoxedValue,
    InlineBytes,
    InlineArray,
    AbstractRef,
}

impl TypeInstanceKindV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::FixedObject => 1,
            Self::BoxedValue => 2,
            Self::InlineBytes => 3,
            Self::InlineArray => 4,
            Self::AbstractRef => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineStorageKindV1 {
    None,
    Inline,
    ZeroSized,
}

impl InlineStorageKindV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Inline => 1,
            Self::ZeroSized => 2,
        }
    }
}

/// Complete, validated semantic shape encoded by `ScoopTypeInstanceShapeV1`.
///
/// The physical scalar fields are private so no caller can manufacture a
/// combination outside the closed M23 matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeInstanceShapeV1 {
    instance_kind: TypeInstanceKindV1,
    inline_storage_kind: InlineStorageKindV1,
    minimum_size: u64,
    instance_alignment: u64,
    inline_offset: u64,
    inline_size: u64,
    inline_stride: u64,
    inline_alignment: u64,
    object_scan: RefScan,
    inline_scan: RefScan,
}

impl TypeInstanceShapeV1 {
    pub fn fixed_object(
        target: LirTargetProfile,
        allocation_size: u64,
        instance_alignment: u64,
        object_scan: RefScan,
    ) -> Result<Self, TypeInstanceShapeError> {
        let allocation_size =
            NonZeroU64::new(allocation_size).ok_or(TypeInstanceShapeError::ZeroAllocationSize)?;
        let instance_alignment = checked_alignment(instance_alignment)?;
        require_managed_alignment(target, instance_alignment)?;
        require_maximum_managed_alignment(target, instance_alignment)?;
        if allocation_size.get() < managed_header_size(target)? {
            return Err(TypeInstanceShapeError::AllocationSmallerThanHeader);
        }
        if allocation_size.get() % instance_alignment.get() != 0 {
            return Err(TypeInstanceShapeError::UnalignedAllocationSize);
        }
        require_managed_object_size(target, allocation_size.get())?;
        let checked_scan =
            CheckedRefScanV1::from_canonical(object_scan).map_err(TypeInstanceShapeError::Scan)?;
        checked_scan
            .validate_extent(allocation_size.get(), instance_alignment.get())
            .map_err(TypeInstanceShapeError::Scan)?;
        Ok(Self {
            instance_kind: TypeInstanceKindV1::FixedObject,
            inline_storage_kind: InlineStorageKindV1::None,
            minimum_size: allocation_size.get(),
            instance_alignment: instance_alignment.get(),
            inline_offset: 0,
            inline_size: 0,
            inline_stride: 0,
            inline_alignment: 0,
            object_scan: checked_scan.into_ref_scan(),
            inline_scan: RefScan::None,
        })
    }

    pub fn boxed_value(
        target: LirTargetProfile,
        value: ValueStorageLayoutV1,
    ) -> Result<Self, TypeInstanceShapeError> {
        let header = managed_header_size(target)?;
        let (inline_storage_kind, inline_size, inline_alignment, inline_scan) = match value.kind() {
            ValueStorageKindV1::ZeroSized { alignment } => {
                (InlineStorageKindV1::ZeroSized, 0, alignment, RefScan::None)
            }
            ValueStorageKindV1::Inline {
                size,
                alignment,
                scan,
            } => (
                InlineStorageKindV1::Inline,
                size.get(),
                alignment,
                scan.as_ref_scan().clone(),
            ),
        };
        require_maximum_managed_alignment(target, checked_alignment(inline_alignment.get())?)?;
        let inline_offset = checked_align_up(header, inline_alignment.get())?;
        let header_alignment = target.metadata_pointer_layout().alignment_bytes().max(
            target
                .scalar_layout(crate::BackendScalarKind::I64)
                .alignment_bytes(),
        );
        let instance_alignment = header_alignment.max(inline_alignment.get());
        let minimum_size = checked_align_up(
            inline_offset
                .checked_add(inline_size)
                .ok_or(TypeInstanceShapeError::SizeOverflow)?,
            instance_alignment,
        )?;
        require_managed_object_size(target, minimum_size)?;
        let object_scan = CheckedRefScanV1::from_canonical(inline_scan.clone())
            .and_then(|scan| scan.translated(inline_offset))
            .map_err(TypeInstanceShapeError::Scan)?
            .into_ref_scan();
        Ok(Self {
            instance_kind: TypeInstanceKindV1::BoxedValue,
            inline_storage_kind,
            minimum_size,
            instance_alignment,
            inline_offset,
            inline_size,
            inline_stride: 0,
            inline_alignment: inline_alignment.get(),
            object_scan,
            inline_scan,
        })
    }

    pub fn inline_bytes(target: LirTargetProfile) -> Result<Self, TypeInstanceShapeError> {
        let prefix = variable_prefix_size(target)?;
        let alignment = managed_header_alignment(target);
        require_managed_object_size(target, prefix)?;
        Ok(Self {
            instance_kind: TypeInstanceKindV1::InlineBytes,
            inline_storage_kind: InlineStorageKindV1::Inline,
            minimum_size: prefix,
            instance_alignment: alignment,
            inline_offset: prefix,
            inline_size: 1,
            inline_stride: 1,
            inline_alignment: 1,
            object_scan: RefScan::None,
            inline_scan: RefScan::None,
        })
    }

    pub fn inline_array(
        target: LirTargetProfile,
        element: ArrayElementStorageV1,
    ) -> Result<Self, TypeInstanceShapeError> {
        let prefix = variable_prefix_size(target)?;
        let (inline_storage_kind, inline_size, inline_stride, inline_alignment, inline_scan) =
            match element.kind() {
                ArrayElementStorageKindV1::ZeroSized { alignment } => (
                    InlineStorageKindV1::ZeroSized,
                    0,
                    0,
                    alignment,
                    RefScan::None,
                ),
                ArrayElementStorageKindV1::Inline {
                    stride,
                    alignment,
                    scan,
                } => (
                    InlineStorageKindV1::Inline,
                    stride.get(),
                    stride.get(),
                    alignment,
                    scan.as_ref_scan().clone(),
                ),
            };
        checked_alignment(inline_alignment.get())?;
        let inline_offset = checked_align_up(prefix, inline_alignment.get())?;
        let instance_alignment = managed_header_alignment(target).max(inline_alignment.get());
        require_maximum_managed_alignment(target, checked_alignment(inline_alignment.get())?)?;
        require_managed_object_size(target, inline_offset)?;
        let object_scan = if inline_scan.contains_reference() {
            RefScan::Array {
                length_offset: managed_header_size(target)?,
                first_element_offset: inline_offset,
                stride: NonZeroU64::new(inline_stride)
                    .ok_or(TypeInstanceShapeError::ZeroArrayScanStride)?,
                element: Box::new(
                    crate::NonEmptyRefScan::new(inline_scan.clone())
                        .expect("a reference-bearing scan is nonempty"),
                ),
            }
        } else {
            RefScan::None
        };
        let checked_object_scan = CheckedRefScanV1::from_canonical(object_scan.clone())
            .map_err(TypeInstanceShapeError::Scan)?;
        checked_object_scan
            .validate_extent(inline_offset, instance_alignment)
            .map_err(TypeInstanceShapeError::Scan)?;
        Ok(Self {
            instance_kind: TypeInstanceKindV1::InlineArray,
            inline_storage_kind,
            minimum_size: inline_offset,
            instance_alignment,
            inline_offset,
            inline_size,
            inline_stride,
            inline_alignment: inline_alignment.get(),
            object_scan,
            inline_scan,
        })
    }

    pub const fn abstract_ref() -> Self {
        Self {
            instance_kind: TypeInstanceKindV1::AbstractRef,
            inline_storage_kind: InlineStorageKindV1::None,
            minimum_size: 0,
            instance_alignment: 0,
            inline_offset: 0,
            inline_size: 0,
            inline_stride: 0,
            inline_alignment: 0,
            object_scan: RefScan::None,
            inline_scan: RefScan::None,
        }
    }

    pub const fn instance_kind(&self) -> TypeInstanceKindV1 {
        self.instance_kind
    }

    pub const fn inline_storage_kind(&self) -> InlineStorageKindV1 {
        self.inline_storage_kind
    }

    pub const fn minimum_size(&self) -> u64 {
        self.minimum_size
    }

    pub const fn instance_alignment(&self) -> u64 {
        self.instance_alignment
    }

    pub const fn inline_offset(&self) -> u64 {
        self.inline_offset
    }

    pub const fn inline_size(&self) -> u64 {
        self.inline_size
    }

    pub const fn inline_stride(&self) -> u64 {
        self.inline_stride
    }

    pub const fn inline_alignment(&self) -> u64 {
        self.inline_alignment
    }

    pub const fn object_scan(&self) -> &RefScan {
        &self.object_scan
    }

    pub const fn inline_scan(&self) -> &RefScan {
        &self.inline_scan
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeInstanceShapeError {
    ZeroAllocationSize,
    ZeroInlineSize,
    ZeroAlignment,
    AlignmentNotPowerOfTwo(u64),
    ManagedAlignmentTooSmall,
    ManagedAlignmentTooLarge { actual: u64, maximum: u64 },
    ManagedObjectTooLarge { actual: u64, maximum: u64 },
    AllocationSmallerThanHeader,
    UnalignedAllocationSize,
    UnalignedInlineSize,
    SizeOverflow,
    ScanOffsetOverflow,
    EmptyReferenceScan,
    EmptySequence,
    EmptySequenceChild,
    ZeroArrayScanStride,
    ArrayStrideMismatch,
    Scan(RefScanValidationError),
}

fn checked_alignment(value: u64) -> Result<NonZeroU64, TypeInstanceShapeError> {
    let value = NonZeroU64::new(value).ok_or(TypeInstanceShapeError::ZeroAlignment)?;
    if value.get().is_power_of_two() {
        Ok(value)
    } else {
        Err(TypeInstanceShapeError::AlignmentNotPowerOfTwo(value.get()))
    }
}

fn checked_align_up(value: u64, alignment: u64) -> Result<u64, TypeInstanceShapeError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or(TypeInstanceShapeError::SizeOverflow)
}

fn managed_header_alignment(target: LirTargetProfile) -> u64 {
    target
        .pointer_layout(PointerKind::Metadata)
        .alignment_bytes()
        .max(
            target
                .scalar_layout(crate::BackendScalarKind::I64)
                .alignment_bytes(),
        )
}

fn require_managed_alignment(
    target: LirTargetProfile,
    alignment: NonZeroU64,
) -> Result<(), TypeInstanceShapeError> {
    if alignment.get() < managed_header_alignment(target) {
        Err(TypeInstanceShapeError::ManagedAlignmentTooSmall)
    } else {
        Ok(())
    }
}

fn require_maximum_managed_alignment(
    target: LirTargetProfile,
    alignment: NonZeroU64,
) -> Result<(), TypeInstanceShapeError> {
    let maximum = target.contract().maximum_managed_alignment();
    if alignment.get() <= maximum {
        Ok(())
    } else {
        Err(TypeInstanceShapeError::ManagedAlignmentTooLarge {
            actual: alignment.get(),
            maximum,
        })
    }
}

fn require_managed_object_size(
    target: LirTargetProfile,
    size: u64,
) -> Result<(), TypeInstanceShapeError> {
    let maximum = target.contract().maximum_managed_object_size();
    if size <= maximum {
        Ok(())
    } else {
        Err(TypeInstanceShapeError::ManagedObjectTooLarge {
            actual: size,
            maximum,
        })
    }
}

fn managed_header_size(target: LirTargetProfile) -> Result<u64, TypeInstanceShapeError> {
    let pointer = target.pointer_layout(PointerKind::Metadata);
    let word = target.scalar_layout(crate::BackendScalarKind::I64);
    let word_offset = checked_align_up(pointer.size_bytes(), word.alignment_bytes())?;
    checked_align_up(
        word_offset
            .checked_add(word.size_bytes())
            .ok_or(TypeInstanceShapeError::SizeOverflow)?,
        pointer.alignment_bytes().max(word.alignment_bytes()),
    )
}

fn variable_prefix_size(target: LirTargetProfile) -> Result<u64, TypeInstanceShapeError> {
    let header = managed_header_size(target)?;
    let word = target.scalar_layout(crate::BackendScalarKind::I64);
    let length_offset = checked_align_up(header, word.alignment_bytes())?;
    checked_align_up(
        length_offset
            .checked_add(word.size_bytes())
            .ok_or(TypeInstanceShapeError::SizeOverflow)?,
        managed_header_alignment(target),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxed_value_derives_object_scan_from_inline_scan() {
        let value = ValueStorageLayoutV1::inline(16, 8, RefScan::References(vec![0, 8])).unwrap();
        let shape =
            TypeInstanceShapeV1::boxed_value(LirTargetProfile::DARWIN_AARCH64, value).unwrap();
        assert_eq!(shape.inline_offset(), 16);
        assert_eq!(shape.minimum_size(), 32);
        assert_eq!(shape.object_scan(), &RefScan::References(vec![16, 24]));
    }

    #[test]
    fn zero_sized_array_keeps_a_nonzero_instance_extent() {
        let element = ArrayElementStorageV1::zero_sized(16).unwrap();
        let shape =
            TypeInstanceShapeV1::inline_array(LirTargetProfile::DARWIN_AARCH64, element).unwrap();
        assert_eq!(shape.inline_storage_kind(), InlineStorageKindV1::ZeroSized);
        assert_eq!(shape.minimum_size(), 32);
        assert_eq!(shape.inline_size(), 0);
        assert_eq!(shape.inline_stride(), 0);
        assert_eq!(shape.object_scan(), &RefScan::None);
    }

    #[test]
    fn abstract_reference_is_the_only_zero_minimum_shape() {
        let shape = TypeInstanceShapeV1::abstract_ref();
        assert_eq!(shape.minimum_size(), 0);
        assert_eq!(shape.instance_alignment(), 0);
        assert_eq!(shape.instance_kind(), TypeInstanceKindV1::AbstractRef);
    }

    #[test]
    fn inline_storage_rejects_implicit_stride_padding() {
        assert_eq!(
            ArrayElementStorageV1::inline(9, 8, RefScan::None),
            Err(TypeInstanceShapeError::UnalignedInlineSize)
        );
        assert_eq!(
            ValueStorageLayoutV1::inline(9, 8, RefScan::None),
            Err(TypeInstanceShapeError::UnalignedInlineSize)
        );
    }

    #[test]
    fn fixed_object_rejects_an_extent_smaller_than_the_header() {
        assert_eq!(
            TypeInstanceShapeV1::fixed_object(
                LirTargetProfile::DARWIN_AARCH64,
                8,
                8,
                RefScan::None,
            ),
            Err(TypeInstanceShapeError::AllocationSmallerThanHeader)
        );
    }
}
