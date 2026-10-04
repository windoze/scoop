use super::*;

pub(super) fn dump_metadata(module: &Module, out: &mut String) {
    for (id, td) in module.meta.type_descriptors.iter() {
        let reference = TypeDescriptorRef::Local(id);
        if reference == module.meta.well_known_type_descriptors.string
            || module
                .meta
                .arrays
                .iter()
                .any(|(_, array)| array.type_descriptor == reference)
        {
            continue;
        }
        let parent = td
            .parent
            .map(type_descriptor_ref_name)
            .unwrap_or_else(|| "none".to_string());
        let vtable = td
            .vtable
            .slots()
            .iter()
            .map(|entry| callable_ref_name(entry.callable))
            .collect::<Vec<_>>()
            .join(", ");
        let itables = td
            .itables
            .iter()
            .map(|record| {
                let slots = record
                    .slots()
                    .iter()
                    .map(|entry| callable_ref_name(entry.callable))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}:[{slots}]", type_descriptor_ref_name(record.interface()))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let function = match &td.relations {
            TypeDescriptorRelations::Absent => String::new(),
            TypeDescriptorRelations::Interface { parents } => format!(
                " parents=[{}]",
                parents
                    .iter()
                    .map(|parent| type_descriptor_ref_name(
                        parent.expect("interface parents are actual descriptors")
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            TypeDescriptorRelations::Signature {
                is_suspend,
                parameters,
                result,
            } => {
                let name = |reference: &Option<TypeDescriptorRef>| {
                    reference
                        .map(type_descriptor_ref_name)
                        .unwrap_or_else(|| "Any".to_string())
                };
                format!(
                    " function={}({})->{}",
                    if *is_suspend { "suspend" } else { "ordinary" },
                    parameters.iter().map(name).collect::<Vec<_>>().join(", "),
                    name(result),
                )
            }
        };
        out.push_str(&format!(
            "  td td{} {} @{} type-id={} shape={:?} minimum-size={} align={} parent={} vtable=[{}] itables=[{}]{function}\n",
            id.into_raw(),
            td.diagnostic_name,
            td.identity.symbol(),
            td.identity.runtime_type().runtime_type().get(),
            td.instance_shape.instance_kind(),
            td.instance_shape.minimum_size(),
            td.instance_shape.instance_alignment(),
            parent,
            vtable,
            itables,
        ));
    }
    for (id, array) in module.meta.arrays.iter() {
        let TypeDescriptorRef::Local(descriptor_id) = array.type_descriptor else {
            unreachable!("a local array application owns a local descriptor")
        };
        let descriptor = &module.meta.type_descriptors[descriptor_id];
        out.push_str(&format!(
            "  array-type array{} {} kind={} element={} size={} align={} scan={} td={}\n",
            id.into_raw(),
            descriptor.diagnostic_name,
            match array.kind {
                ArrayKind::Immutable => "immutable",
                ArrayKind::Mutable => "mutable",
            },
            array.element.dump(),
            array.layout.instance().inline_size(),
            array.layout.instance().inline_alignment(),
            descriptor.instance_shape.inline_scan().dump(),
            type_descriptor_ref_name(array.type_descriptor),
        ));
    }
    // String remains first, followed by every exact source integer layout in
    // arena order, Boolean, and ordinary layouts.
    for (_layout_id, layout) in module
        .meta
        .layouts
        .iter()
        .filter(|(_, layout)| {
            matches!(
                layout.kind,
                LayoutKind::Intrinsic(IntrinsicTypeRepresentation::String)
            )
        })
        .chain(module.meta.layouts.iter().filter(|(_, layout)| {
            matches!(
                layout.kind,
                LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Integer(_))
            )
        }))
        .chain(module.meta.layouts.iter().filter(|(_, layout)| {
            matches!(
                layout.kind,
                LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Boolean)
            )
        }))
        .chain(module.meta.layouts.iter().filter(|(_, layout)| {
            !matches!(
                layout.kind,
                LayoutKind::Intrinsic(
                    IntrinsicTypeRepresentation::Integer(_)
                        | IntrinsicTypeRepresentation::Char
                        | IntrinsicTypeRepresentation::Boolean
                        | IntrinsicTypeRepresentation::String
                )
            )
        }))
    {
        match &layout.kind {
            LayoutKind::Plain { scan } => match scan {
                RefScan::None => out.push_str(&format!(
                    "  layout {} size={} align={} refs=[]\n",
                    layout.name, layout.size, layout.align
                )),
                RefScan::References(offsets) => out.push_str(&format!(
                    "  layout {} size={} align={} refs={offsets:?}\n",
                    layout.name, layout.size, layout.align
                )),
                _ => out.push_str(&format!(
                    "  layout {} size={} align={} scan={}\n",
                    layout.name,
                    layout.size,
                    layout.align,
                    scan.dump()
                )),
            },
            LayoutKind::Enum { scan } => out.push_str(&format!(
                "  layout {} size={} align={} enum-scan={}\n",
                layout.name,
                layout.size,
                layout.align,
                scan.dump()
            )),
            LayoutKind::Intrinsic(
                IntrinsicTypeRepresentation::Integer(_)
                | IntrinsicTypeRepresentation::Char
                | IntrinsicTypeRepresentation::Boolean
                | IntrinsicTypeRepresentation::String,
            ) => out.push_str(&format!(
                "  layout {} size={} align={} refs=[]\n",
                layout.name, layout.size, layout.align
            )),
            LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Ptr { pointee }) => {
                let pointee = match pointee {
                    LirDataPointee::OpaqueVoid => "void".to_string(),
                    LirDataPointee::Value(ty) => ty.dump(),
                };
                out.push_str(&format!(
                    "  layout {} size={} align={} intrinsic=ptr<{}> refs=[]\n",
                    layout.name, layout.size, layout.align, pointee
                ));
            }
            LayoutKind::Intrinsic(IntrinsicTypeRepresentation::FunPtr { signature }) => {
                let params = signature
                    .params
                    .iter()
                    .map(LirType::dump)
                    .collect::<Vec<_>>()
                    .join(",");
                let result = match &signature.return_type {
                    LirReturnType::Void => "void".to_string(),
                    LirReturnType::Value(ty) => ty.dump(),
                };
                out.push_str(&format!(
                    "  layout {} size={} align={} intrinsic=funptr<({})->{}> refs=[]\n",
                    layout.name, layout.size, layout.align, params, result
                ));
            }
        }
        if let Some(c_layout) = layout.c_layout {
            let fields = layout
                .fields
                .iter()
                .map(|field| format!("{}@{}", field.offset, field.access_align))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "  layout-meta {} c-layout(aligned={},packed={}) fields=[{}] interior-mutable={}\n",
                layout.name,
                c_layout.aligned.bytes().unwrap_or(0),
                c_layout.packed.bytes().unwrap_or(0),
                fields,
                layout.interior_mutable
            ));
        } else if layout.interior_mutable {
            out.push_str(&format!(
                "  layout-meta {} interior-mutable=true\n",
                layout.name
            ));
        }
    }
}
