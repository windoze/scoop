use crate::{
    AtomicValueKind, BackendScalarKind, FieldLayout, LirTargetProfile, RefScan, ScalarLayout,
    StorageGeometryV1, StorageLayoutCursorV1, StoragePlacementPolicyV1, TypeInstanceShapeError,
};

pub struct AtomicObjectLayoutV1 {
    pub value: FieldLayout,
    pub size: u64,
    pub align: u64,
    pub scan: RefScan,
}

/// The runtime header followed by one naturally aligned hidden value field.
pub fn atomic_object_layout(
    target: LirTargetProfile,
    kind: AtomicValueKind,
) -> Result<AtomicObjectLayoutV1, TypeInstanceShapeError> {
    let geometry = |layout: ScalarLayout| {
        StorageGeometryV1::new(target, layout.size_bytes(), layout.alignment_bytes())
    };
    let mut header = StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::Ordinary)?;
    header.push(geometry(target.metadata_pointer_layout())?)?;
    header.push(geometry(target.scalar_layout(BackendScalarKind::I64))?)?;
    let mut object = StorageLayoutCursorV1::with_prefix(header.finish()?);
    let field = match kind {
        AtomicValueKind::Int => target.scalar_layout(BackendScalarKind::I32),
        AtomicValueKind::Long => target.scalar_layout(BackendScalarKind::I64),
        AtomicValueKind::Boolean => target.scalar_layout(BackendScalarKind::I8),
        AtomicValueKind::Reference => target.managed_pointer_layout(),
    };
    let field = object.push(geometry(field)?)?;
    let object = object.finish()?;
    Ok(AtomicObjectLayoutV1 {
        value: FieldLayout {
            offset: field.offset(),
            access_align: field.access_alignment().get(),
        },
        size: object.size(),
        align: object.alignment().get(),
        scan: if kind == AtomicValueKind::Reference {
            RefScan::References(vec![field.offset()])
        } else {
            RefScan::None
        },
    })
}
