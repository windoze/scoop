use super::*;

/// Fix the representation of every MIR enum definition (spec 7.4).
pub(crate) fn lower_enums(module: &mir::Module) -> Arena<lir::EnumDef> {
    let mut reprs: Vec<Option<lir::EnumRepr>> = Vec::new();
    reprs.resize_with(module.enums.len(), || None);
    for (id, _) in module.enums.iter() {
        compute_repr(module, &mut reprs, id);
    }
    let mut enums = Arena::new();
    for ((_, def), repr) in module.enums.iter().zip(reprs) {
        enums.alloc(lir::EnumDef {
            name: def.name.clone(),
            repr: repr.expect("compute_repr fills every entry"),
            scan: lir::RefScan::None,
        });
    }
    for (id, _) in module.enums.iter() {
        let scan = ref_scan(module, &enums, &mir::Type::Enum(id, Vec::new()), 0);
        enums[enum_def_id(id)].scan = scan;
    }
    enums
}

/// Every enum type nested inside `ty` (through tuple elements and
/// struct fields), for representation sizing.
pub(crate) fn nested_enums(module: &mir::Module, ty: &mir::Type, out: &mut Vec<mir::EnumId>) {
    match ty {
        mir::Type::Enum(id, _) => out.push(*id),
        mir::Type::Tuple(elements) => {
            for element in elements {
                nested_enums(module, element, out);
            }
        }
        mir::Type::Struct(id) => match &module.structs[*id].representation {
            mir::StructRepresentation::Declared { fields, .. } => {
                for field in fields {
                    nested_enums(module, &field.ty, out);
                }
            }
            mir::StructRepresentation::Intrinsic(_) => {}
        },
        // References hide whatever they point at behind a pointer.
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::UInt
        | mir::Type::Boolean
        | mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_)
        | mir::Type::Any => {}
    }
}

pub(crate) fn is_niche_payload(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Function(_)
            | mir::Type::Any
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_)
    )
}

/// Compute (memoized) the representation of one enum. Niche layout is
/// restricted to the exact Option-isomorphic cases from spec 7.4.
/// Tagged layout gives all GC-free variants one shared payload region
/// and every non-GC-free variant its own disjoint slot. Nested enums are
/// computed first because variant sizing needs their shapes.
pub(crate) fn compute_repr(
    module: &mir::Module,
    reprs: &mut Vec<Option<lir::EnumRepr>>,
    id: mir::EnumId,
) {
    let index = id.into_raw().into_u32() as usize;
    if reprs[index].is_some() {
        return;
    }
    let def = &module.enums[id];
    let mut nested = Vec::new();
    for variant in &def.variants {
        for field in &variant.fields {
            nested_enums(module, &field.ty, &mut nested);
        }
    }
    for nested_id in nested {
        // A by-value recursive enum is infinitely sized; hir-lower
        // rejects it before this stage.
        assert!(nested_id != id, "a by-value recursive enum is unsized");
        compute_repr(module, reprs, nested_id);
    }

    // This is a semantic whitelist, not merely an LLVM pointer-shape
    // check: only managed refs, Ptr and FunPtr qualify.
    if def.variants.len() == 2 {
        let has_unit = def.variants.iter().any(|variant| variant.fields.is_empty());
        let payload = def
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| !variant.fields.is_empty());
        if let (true, Some((payload_index, payload_variant))) = (has_unit, payload) {
            if payload_variant.fields.len() == 1 && is_niche_payload(&payload_variant.fields[0].ty)
            {
                reprs[index] = Some(lir::EnumRepr::Niche {
                    payload_variant: payload_index as u32,
                });
                return;
            }
        }
    }

    // First compute each variant's natural field layout independent of
    // its eventual slot assignment.
    let enum_shape = |id: mir::EnumId| {
        repr_shape(
            reprs[id.into_raw().into_u32() as usize]
                .as_ref()
                .expect("nested enum representations are computed first"),
        )
    };
    struct PendingField {
        ty: lir::LirType,
        relative_offset: u64,
    }

    struct PendingVariant {
        fields: Vec<PendingField>,
        size: u64,
        align: u64,
        gc_free: bool,
    }

    let mut pending = Vec::new();
    for variant in &def.variants {
        let fields: Vec<lir::LirType> = variant
            .fields
            .iter()
            .map(|field| lir_type(&field.ty))
            .collect();
        let field_types: Vec<mir::Type> = variant
            .fields
            .iter()
            .map(|field| field.ty.clone())
            .collect();
        let (field_offsets, size, align) = aggregate_shape(module, &enum_shape, &field_types);
        assert_eq!(
            fields.len(),
            field_offsets.len(),
            "aggregate layout returns one offset per enum field"
        );
        pending.push(PendingVariant {
            fields: fields
                .into_iter()
                .zip(field_offsets)
                .map(|(ty, relative_offset)| PendingField {
                    ty,
                    relative_offset,
                })
                .collect(),
            size,
            align,
            gc_free: variant.gc_free,
        });
    }

    // Pure-value variants all reuse this one region. Empty variants need
    // no bytes but retain the same offset in the structural metadata.
    let pure_size = pending
        .iter()
        .filter(|variant| variant.gc_free)
        .map(|variant| variant.size)
        .max()
        .unwrap_or(0);
    let pure_align = pending
        .iter()
        .filter(|variant| variant.gc_free)
        .map(|variant| variant.align)
        .max()
        .unwrap_or(1);
    let pure_offset = 8u64.next_multiple_of(pure_align);
    let mut cursor = pure_offset + pure_size;
    let mut align = 8u64.max(pure_align);
    let mut variants = Vec::with_capacity(pending.len());
    for variant in pending {
        let slot_offset = if !variant.gc_free {
            cursor = cursor.next_multiple_of(variant.align);
            let offset = cursor;
            cursor += variant.size;
            offset
        } else {
            pure_offset
        };
        align = align.max(variant.align);
        variants.push(lir::EnumVariantRepr {
            fields: variant
                .fields
                .into_iter()
                .map(|field| lir::EnumFieldRepr {
                    ty: field.ty,
                    offset: slot_offset + field.relative_offset,
                })
                .collect(),
            slot_offset,
            slot_size: variant.size,
            slot_align: variant.align,
            gc_free: variant.gc_free,
        });
    }
    let size = cursor.next_multiple_of(align);
    reprs[index] = Some(lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    });
}

/// Size and alignment of an enum value from its representation: the
/// niche form is a bare pointer; tagged size/alignment are fixed by its
/// shared pure-value region and disjoint ref-bearing slots.
pub(crate) fn repr_shape(repr: &lir::EnumRepr) -> (u64, u64) {
    match repr {
        lir::EnumRepr::Niche { .. } => (8, 8),
        lir::EnumRepr::Tagged { size, align, .. } => (*size, *align),
    }
}
