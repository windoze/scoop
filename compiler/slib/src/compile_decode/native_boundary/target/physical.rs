use super::*;

#[derive(Clone, Copy)]
pub(super) struct PhysicalType {
    pub(super) size: u64,
    pub(super) alignment: u64,
    pub(super) shape: ScoopAbiValueShape,
    pub(super) gc_free: bool,
}

pub(super) fn scalar(layout: scoop_lir::ScalarLayout, gc_free: bool) -> PhysicalType {
    PhysicalType {
        size: layout.size_bytes(),
        alignment: layout.alignment_bytes(),
        shape: ScoopAbiValueShape::Scalar,
        gc_free,
    }
}

pub(super) fn integer_scalar_kind(
    bit_width: scoop_identity::IntegerBitWidth,
) -> scoop_lir::BackendScalarKind {
    match bit_width {
        scoop_identity::IntegerBitWidth::Bits8 => scoop_lir::BackendScalarKind::I8,
        scoop_identity::IntegerBitWidth::Bits16 => scoop_lir::BackendScalarKind::I16,
        scoop_identity::IntegerBitWidth::Bits32 => scoop_lir::BackendScalarKind::I32,
        scoop_identity::IntegerBitWidth::Bits64 => scoop_lir::BackendScalarKind::I64,
    }
}

pub(super) fn pointer(
    target: scoop_lir::LirTargetProfile,
    kind: scoop_lir::PointerKind,
    gc_free: bool,
) -> PhysicalType {
    let layout = target.pointer_layout(kind);
    PhysicalType {
        size: layout.size_bytes(),
        alignment: layout.alignment_bytes(),
        shape: ScoopAbiValueShape::Scalar,
        gc_free,
    }
}

pub(super) fn aggregate(
    fields: &[PhysicalType],
    shape: ScoopAbiValueShape,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    aggregate_with_overrides(fields, None, None, shape, exact)
}

pub(super) fn aggregate_with_policy(
    fields: &[PhysicalType],
    policy: NativeBoundaryCLayoutPolicy,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    match policy {
        NativeBoundaryCLayoutPolicy::NotCLayout => {
            aggregate_with_overrides(fields, None, None, ScoopAbiValueShape::Aggregate, exact)
        }
        NativeBoundaryCLayoutPolicy::CLayout { aligned, packed } => aggregate_with_overrides(
            fields,
            override_bytes(aligned),
            override_bytes(packed),
            ScoopAbiValueShape::Aggregate,
            exact,
        ),
    }
}

pub(super) fn aggregate_with_overrides(
    fields: &[PhysicalType],
    aligned: Option<u64>,
    packed: Option<u64>,
    shape: ScoopAbiValueShape,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    aggregate_values(fields.iter().copied(), aligned, packed, shape, exact)
}

pub(super) fn aggregate_values(
    fields: impl IntoIterator<Item = PhysicalType>,
    aligned: Option<u64>,
    packed: Option<u64>,
    shape: ScoopAbiValueShape,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    let mut size = 0_u64;
    let mut alignment = aligned.unwrap_or(1);
    let mut gc_free = true;
    for field in fields {
        let access_alignment = packed.map_or(field.alignment, |cap| field.alignment.min(cap));
        size = align_up(size, access_alignment)?;
        size = size
            .checked_add(field.size)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        alignment = alignment.max(access_alignment);
        gc_free &= field.gc_free;
    }
    Ok(PhysicalType {
        size: align_up(size, alignment)?,
        alignment,
        shape,
        gc_free,
    })
}

pub(super) fn align_up(value: u64, alignment: u64) -> Result<u64, NativeBoundaryCompileError> {
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(NativeBoundaryTargetError::ArithmeticOverflow.into())
    }
}

pub(super) fn override_bytes(value: scoop_identity::CLayoutOverride) -> Option<u64> {
    match value {
        scoop_identity::CLayoutOverride::Natural => None,
        scoop_identity::CLayoutOverride::Bytes(bytes) => Some(u64::from(bytes.get())),
    }
}
