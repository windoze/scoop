use super::*;
use scoop_wire::WirePath;

fn c_nullable_option_kind(module: &mir::Module, id: mir::EnumId) -> Option<lir::NichePointerKind> {
    let option = module.option_core(id)?;
    assert_eq!(option.enum_id(), id, "Option refinement has exact identity");
    let some = option
        .some()
        .definition(&module.enums)
        .expect("core Option Some variant exists");
    let none = option
        .none()
        .definition(&module.enums)
        .expect("core Option None variant exists");
    let payload = option
        .some_payload()
        .definition(&module.enums)
        .expect("core Option Some payload exists");
    assert_eq!(some.fields.len(), 1, "core Option Some has one field");
    assert!(none.fields.is_empty(), "core Option None has no fields");
    match payload.ty {
        mir::Type::Ptr(_) => Some(lir::NichePointerKind::Raw),
        mir::Type::FunPtr(_) => Some(lir::NichePointerKind::Code),
        _ => None,
    }
}

/// Fix the representation of every MIR enum definition (spec 7.4).
pub(crate) fn lower_enums(
    context: &LoweringContext,
    module: &mir::Module,
) -> StorageResult<lir::EnumDefs> {
    let mut reprs: Vec<Option<lir::EnumRepr>> = Vec::new();
    reprs.resize_with(module.enums.len(), || None);
    let mut visiting = std::collections::HashSet::new();

    for (id, _) in module.enums.iter() {
        compute_repr(context, module, &mut reprs, &mut visiting, id)?;
    }
    let mut enums = lir::EnumDefs::default();
    for ((mir_id, def), repr) in module.enums.iter().zip(reprs) {
        let definition = lir::EnumDef {
            exact_type: exact_type_record(
                module,
                &mir::Type::Enum(mir_id, def.type_arguments.clone()),
            )
            .id(),
            name: def.name.clone(),
            repr: repr.ok_or(StorageLoweringError::InvalidRepresentation(
                "enum representation is incomplete",
            ))?,
            scan: lir::RefScan::None,
        };
        let lir_id = match c_nullable_option_kind(module, mir_id) {
            Some(lir::NichePointerKind::Raw) => {
                enums.alloc_c_nullable_data_pointer_option(definition)
            }
            Some(lir::NichePointerKind::Code) => {
                enums.alloc_c_nullable_code_pointer_option(definition)
            }
            Some(lir::NichePointerKind::Managed) => {
                unreachable!("C-nullable Option payloads are raw or code pointers")
            }
            None => enums.alloc(definition),
        };
        assert_eq!(
            lir_id,
            enum_def_id(mir_id),
            "MIR and LIR enum definition stores remain index-aligned",
        );
    }
    for (id, _) in module.enums.iter() {
        let scan = ref_scan(context, module, &enums, &mir::Type::Enum(id, Vec::new()), 0)?;
        let (size, alignment) = repr_shape(context, &enums[enum_def_id(id)].repr);
        value_storage(context, size, alignment, scan.clone())?;
        enums.set_scan(enum_def_id(id), scan);
    }
    Ok(enums)
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
        mir::Type::Context(_)
        | mir::Type::Unit
        | mir::Type::Integer(_)
        | mir::Type::MachineScalar(_)
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

fn niche_pointer_kind(ty: &mir::Type) -> lir::NichePointerKind {
    match ty {
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::NichePointerKind::Managed,
        mir::Type::Ptr(_) => lir::NichePointerKind::Raw,
        mir::Type::FunPtr(_) => lir::NichePointerKind::Code,
        _ => unreachable!("only pointer-like fields qualify for a niche enum"),
    }
}

/// Compute (memoized) the representation of one enum. Niche layout is
/// restricted to the exact Option-isomorphic cases from spec 7.4.
/// Tagged layout gives all GC-free variants one shared payload region
/// and every non-GC-free variant its own disjoint slot. Nested enums are
/// computed first because variant sizing needs their shapes.
pub(crate) fn compute_repr(
    context: &LoweringContext,
    module: &mir::Module,
    reprs: &mut Vec<Option<lir::EnumRepr>>,
    visiting: &mut std::collections::HashSet<mir::EnumId>,
    id: mir::EnumId,
) -> StorageResult<()> {
    let index = id.into_raw().into_u32() as usize;
    if reprs[index].is_some() {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(StorageLoweringError::InvalidRepresentation(
            "by-value enum cycle",
        ));
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
        compute_repr(context, module, reprs, visiting, nested_id)?;
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
                    kind: niche_pointer_kind(&payload_variant.fields[0].ty),
                    payload_variant: payload_index as u32,
                });
                visiting.remove(&id);
                return Ok(());
            }
        }
    }

    // First compute each variant's natural field layout independent of
    // its eventual slot assignment.
    let enum_shape = |id: mir::EnumId| {
        let repr = reprs[id.into_raw().into_u32() as usize].as_ref().ok_or(
            StorageLoweringError::InvalidRepresentation("nested enum representation is incomplete"),
        )?;
        Ok(repr_shape(context, repr))
    };
    reprs[index] = Some(tagged_repr(context, module, &enum_shape, def)?);
    visiting.remove(&id);
    Ok(())
}

fn tagged_repr(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> StorageResult<(u64, u64)>,
    definition: &mir::EnumDef,
) -> StorageResult<lir::EnumRepr> {
    let mut field_geometries = reserve(definition.variants.len())?;
    for variant in &definition.variants {
        let mut fields = reserve(variant.fields.len())?;
        for field in &variant.fields {
            let (size, alignment) = size_align(context, module, enum_shape, &field.ty)?;
            fields.push(lir::StorageGeometryV1::new(
                context.target_profile(),
                size,
                alignment,
            )?);
        }
        field_geometries.push(fields);
    }
    let mut inputs = reserve(definition.variants.len())?;
    for (variant, fields) in definition.variants.iter().zip(&field_geometries) {
        inputs.push(lir::EnumVariantGeometryInputV1 {
            fields,
            gc_free: variant.gc_free,
        });
    }
    let geometry = lir::EnumStorageGeometryV1::tagged(context.target_profile(), &inputs)?;
    let mut variants = reserve(definition.variants.len())?;
    for (source, variant) in definition.variants.iter().zip(geometry.variants()) {
        let mut fields = reserve(source.fields.len())?;
        for (source, field) in source.fields.iter().zip(variant.fields()) {
            fields.push(lir::EnumFieldRepr {
                ty: lir_type(module, &source.ty),
                offset: field.offset(),
            });
        }
        variants.push(lir::EnumVariantRepr {
            fields,
            slot_offset: variant.slot().region().offset(),
            slot_size: variant.storage().size(),
            slot_align: variant.storage().alignment().get(),
            gc_free: source.gc_free,
        });
    }
    Ok(lir::EnumRepr::Tagged {
        variants,
        size: geometry.storage().size(),
        align: geometry.storage().alignment().get(),
    })
}

fn reserve<T>(length: usize) -> StorageResult<Vec<T>> {
    let mut values = Vec::new();
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(&mut values, length, &path)
        .map_err(lir::EnumStorageGeometryErrorV1::Resource)?;
    Ok(values)
}

/// Size and alignment of an enum value from its representation: the
/// niche form is a bare pointer; tagged size/alignment are fixed by its
/// shared pure-value region and disjoint ref-bearing slots.
pub(crate) fn repr_shape(context: &LoweringContext, repr: &lir::EnumRepr) -> (u64, u64) {
    match repr {
        lir::EnumRepr::Niche { kind, .. } => {
            let layout = context.pointer_layout(kind.pointer_kind());
            (layout.size, layout.align)
        }
        lir::EnumRepr::Tagged { size, align, .. } => (*size, *align),
    }
}
