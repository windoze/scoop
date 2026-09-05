use super::*;
mod objects;
mod shapes;
mod values;

pub(crate) use objects::*;
pub(crate) use shapes::*;
pub(crate) use values::*;

/// The meta layouts (DESIGN 2.4 / 3.4): the runtime `String` object
/// header, the `Int` / `Boolean` scalars, every struct in declaration
/// order, every enum in declaration order (with fixed reference
/// offsets), every class in declaration order (M6: header + fields),
/// and every tuple type that appears in the module. Concrete arrays have a
/// separate, typed metadata arena rather than a second layout identity.
pub(crate) fn layouts(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    from_code: &[mir::Type],
) -> (Arena<lir::Layout>, lir::WellKnownLayouts) {
    // Tuple types reachable from struct / enum / class declarations
    // appear even when no code value mentions them directly.
    let mut types = Vec::new();
    for (_, def) in module.structs.iter() {
        match &def.representation {
            mir::StructRepresentation::Declared { fields, .. } => {
                for field in fields {
                    record_layout_types(&field.ty, &mut types);
                }
            }
            mir::StructRepresentation::Intrinsic(_) => {}
        }
    }
    for (_, def) in module.enums.iter() {
        for variant in &def.variants {
            for field in &variant.fields {
                record_layout_types(&field.ty, &mut types);
            }
        }
    }
    for (_, def) in module.classes.iter() {
        match &def.representation {
            mir::ClassRepresentation::Declared { fields, .. } => {
                for field in fields {
                    record_layout_types(&field.ty, &mut types);
                }
            }
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { element }
                | mir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => record_layout_types(element, &mut types),
            mir::ClassRepresentation::Intrinsic(_) => {}
        }
    }
    for (_, def) in module.closure_classes.iter() {
        for field in &def.captures {
            record_layout_types(&field.ty, &mut types);
        }
    }
    for ty in from_code {
        record_layout_types(ty, &mut types);
    }

    let mut layouts = Arena::new();
    for (_, def) in module.structs.iter() {
        layouts.alloc(struct_layout(context, module, enums, def));
    }
    for (id, def) in module.enums.iter() {
        layouts.alloc(enum_layout(context, enums, id, def));
    }
    let mut string = None;
    for (_, def) in module.classes.iter() {
        match def.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
                let layout = class_definition_layout(context, module, enums, def);
                assert!(
                    string.replace(layouts.alloc(layout)).is_none(),
                    "one typed String representation"
                );
            }
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. },
            ) => {}
            _ => {
                layouts.alloc(class_definition_layout(context, module, enums, def));
            }
        }
    }
    for (_, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def);
        layouts.alloc(lir::Layout {
            name: def.name.clone(),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Plain { scan },
        });
    }
    // Tuple layouts keep their first-appearance order.
    for ty in &types {
        if let mir::Type::Tuple(elements) = ty {
            layouts.alloc(aggregate_layout(
                context,
                module,
                enums,
                mir::type_name(module, ty),
                elements,
            ));
        }
    }
    (
        layouts,
        lir::WellKnownLayouts {
            string: string
                .expect("LocalConcreteHir supplies the typed intrinsic String representation"),
        },
    )
}
