use super::*;

/// Normalize a list of scans: remove empty parts, flatten sequences,
/// and merge plain reference lists.
pub(crate) fn sequence(parts: impl IntoIterator<Item = lir::RefScan>) -> lir::RefScan {
    fn collect(scan: lir::RefScan, refs: &mut Vec<u64>) {
        match scan {
            lir::RefScan::None => {}
            lir::RefScan::References(offsets) => refs.extend(offsets),
            lir::RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, refs);
                }
            }
        }
    }

    let mut refs = Vec::new();
    for part in parts {
        collect(part, &mut refs);
    }
    if refs.is_empty() {
        lir::RefScan::None
    } else {
        lir::RefScan::References(refs)
    }
}

/// Scan program for `fields` laid out at `offsets`, shifted by `base`.
pub(crate) fn scan_fields(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    fields: &[mir::Type],
    offsets: &[u64],
    base: u64,
) -> lir::RefScan {
    sequence(
        fields
            .iter()
            .zip(offsets)
            .map(|(field, offset)| ref_scan(context, module, enums, field, base + offset)),
    )
}

/// Recursive scan program for one inline value at `base`.
pub(crate) fn ref_scan(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    ty: &mir::Type,
    base: u64,
) -> lir::RefScan {
    match ty {
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::RefScan::References(vec![base]),
        mir::Type::Struct(id) => {
            let fields: Vec<mir::Type> = module.structs[*id]
                .declared_fields()
                .iter()
                .map(|field| field.ty.clone())
                .collect();
            let enum_shape = |id: mir::EnumId| repr_shape(context, &enums[enum_def_id(id)].repr);
            let (field_layouts, _, _) =
                struct_shape(context, module, &enum_shape, &module.structs[*id]);
            let offsets: Vec<_> = field_layouts.iter().map(|field| field.offset).collect();
            scan_fields(context, module, enums, &fields, &offsets, base)
        }
        mir::Type::Tuple(fields) => {
            let enum_shape = |id: mir::EnumId| repr_shape(context, &enums[enum_def_id(id)].repr);
            let (offsets, _, _) = aggregate_shape(context, module, &enum_shape, fields);
            scan_fields(context, module, enums, fields, &offsets, base)
        }
        mir::Type::Enum(id, _) => match &enums[enum_def_id(*id)].repr {
            lir::EnumRepr::Niche {
                kind: lir::NichePointerKind::Managed,
                ..
            } => lir::RefScan::References(vec![base]),
            lir::EnumRepr::Niche {
                kind: lir::NichePointerKind::Raw | lir::NichePointerKind::Code,
                ..
            } => lir::RefScan::None,
            lir::EnumRepr::Tagged { variants, .. } => sequence(
                module.enums[*id]
                    .variants
                    .iter()
                    .zip(variants)
                    .filter(|(_, repr)| !repr.gc_free)
                    .map(|(variant, repr)| {
                        let fields: Vec<mir::Type> = variant
                            .fields
                            .iter()
                            .map(|field| field.ty.clone())
                            .collect();
                        let offsets: Vec<u64> =
                            repr.fields.iter().map(|field| field.offset).collect();
                        scan_fields(context, module, enums, &fields, &offsets, base)
                    }),
            ),
        },
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::UInt
        | mir::Type::MachineScalar(_)
        | mir::Type::Boolean
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => lir::RefScan::None,
    }
}
