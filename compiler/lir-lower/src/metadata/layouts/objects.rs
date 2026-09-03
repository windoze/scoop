use super::*;

/// Layout of a class object (M6, runtime spec 2.1/2.2): the 16-byte
/// object header (M9: TD pointer + GC word) followed by the fields
/// — mir-lower already flattened the base-class prefix into
/// `ClassDef::fields`. Returns size, align, and the reference offsets
/// relative to the object start (the header itself is not a scanned
/// reference). Boxed value types use the same shape: header + the
/// inline payload field.
pub(crate) fn class_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (u64, u64, lir::RefScan) {
    match &def.representation {
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
            return (24, 8, lir::RefScan::None);
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { element }
            | mir::IntrinsicTypeRepresentation::MutableArray { element },
        ) => {
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (size, align) = size_align(module, &enum_shape, element);
            return (
                size.next_multiple_of(align),
                align,
                ref_scan(module, enums, element, 0),
            );
        }
        mir::ClassRepresentation::Intrinsic(_) => {
            unreachable!("the registry fixes intrinsic declaration targets")
        }
        mir::ClassRepresentation::Declared { .. } => {}
    }
    let (offsets, size, align) = class_shape(module, enums, def);
    let fields: Vec<mir::Type> = def
        .declared_fields()
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let scan = scan_fields(module, enums, &fields, &offsets, 0);
    (size, align, scan)
}

/// Natural object layout for one flattened class: the header occupies
/// bytes 0..16 and each base/derived field starts at the next address
/// satisfying its own alignment. LIR heap operations consume these
/// byte offsets directly, so sub-word fields are not rounded to slots.
pub(crate) fn class_shape(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (Vec<u64>, u64, u64) {
    let fields = def.declared_fields();
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 16u64;
    let mut align = 8u64;
    for field in fields {
        let (field_size, field_align) = size_align(module, &enum_shape, &field.ty);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

pub(crate) fn class_definition_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> lir::Layout {
    match &def.representation {
        mir::ClassRepresentation::Declared { fields, .. } => {
            let (size, align, scan) = class_layout(module, enums, def);
            let offsets = class_shape(module, enums, def).0;
            lir::Layout {
                name: def.name.clone(),
                size,
                align,
                fields: fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| {
                        let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
                        let (_, access_align) = size_align(module, &enum_shape, &field.ty);
                        lir::FieldLayout {
                            offset,
                            access_align,
                        }
                    })
                    .collect(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Plain { scan },
            }
        }
        mir::ClassRepresentation::Intrinsic(representation) => {
            let (size, align, kind) = match representation {
                mir::IntrinsicTypeRepresentation::String => {
                    (24, 8, lir::IntrinsicTypeRepresentation::String)
                }
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                    unreachable!("intrinsic arrays use the typed ArrayType metadata arena")
                }
                mir::IntrinsicTypeRepresentation::Int
                | mir::IntrinsicTypeRepresentation::UInt
                | mir::IntrinsicTypeRepresentation::Boolean => {
                    unreachable!("the registry fixes intrinsic declaration targets")
                }
            };
            lir::Layout {
                name: def.name.clone(),
                size,
                align,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Intrinsic(kind),
            }
        }
    }
}

/// Closure object layout: the 16-byte managed header, one non-scanned code
/// pointer at offset 16, then naturally aligned inline capture fields.
pub(crate) fn closure_shape(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClosureClass,
) -> (Vec<u64>, u64, u64, lir::RefScan) {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(def.captures.len());
    let mut size = 24u64;
    let mut align = 8u64;
    for capture in &def.captures {
        let (capture_size, capture_align) = size_align(module, &enum_shape, &capture.ty);
        let offset = size.next_multiple_of(capture_align);
        offsets.push(offset);
        size = offset + capture_size;
        align = align.max(capture_align);
    }
    let fields: Vec<_> = def
        .captures
        .iter()
        .map(|capture| capture.ty.clone())
        .collect();
    let scan = scan_fields(module, enums, &fields, &offsets, 0);
    (offsets, size.next_multiple_of(align), align, scan)
}
