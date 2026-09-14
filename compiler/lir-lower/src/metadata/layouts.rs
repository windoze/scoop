use super::*;
mod objects;
mod shapes;
mod values;

pub(crate) use objects::*;
pub(crate) use shapes::*;
pub(crate) use values::*;

/// Persistent layouts selected by the sealed strong materialization plan.
/// Generic and structural types still participate in physical shape
/// calculation, but never acquire independent LIR layout identities.
pub(crate) fn layouts(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
) -> (Arena<lir::Layout>, lir::WellKnownLayouts) {
    let mut layouts = Arena::new();
    for (id, def) in module.structs.iter() {
        let ty = def.physical_type(id);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        layouts.alloc(struct_layout(
            context,
            module,
            enums,
            struct_layout_identity(context, identity_roots, module, &ty, def),
            def,
        ));
    }
    for (id, def) in module.enums.iter() {
        let ty = mir::Type::Enum(id, def.type_arguments.clone());
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        layouts.alloc(enum_layout(
            context,
            enums,
            id,
            managed_value_layout_identity(context, identity_roots, module, &ty),
            def,
        ));
    }
    let mut string = None;
    for (id, def) in module.classes.iter() {
        let ty = if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            mir::Type::String
        } else {
            mir::Type::Class(id)
        };
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        match def.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
                let layout = class_definition_layout(
                    context,
                    module,
                    enums,
                    managed_object_layout_identity(context, identity_roots, module, &ty),
                    def,
                );
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
                layouts.alloc(class_definition_layout(
                    context,
                    module,
                    enums,
                    managed_object_layout_identity(context, identity_roots, module, &ty),
                    def,
                ));
            }
        }
    }
    for (id, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def);
        layouts.alloc(lir::Layout {
            identity: lir::LayoutIdentity::managed_object(
                generated_exact_type_record(module, mir::GeneratedExactTypeLocation::Closure(id))
                    .id(),
                context.target_profile(),
                identity_roots.for_generated(mir::GeneratedExactTypeLocation::Closure(id)),
            )
            .expect("validated exact type and target must derive a layout identity"),
            name: def.name.clone(),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Plain { scan },
        });
    }
    (
        layouts,
        lir::WellKnownLayouts {
            string: string
                .expect("LocalConcreteHir supplies the typed intrinsic String representation"),
        },
    )
}

fn struct_layout_identity(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    ty: &mir::Type,
    def: &mir::StructDef,
) -> lir::LayoutIdentity {
    let exact_type = exact_type_record(module, ty).id();
    let target_profile = context.target_profile();
    let root = identity_roots.for_type(ty);
    match &def.representation {
        mir::StructRepresentation::Declared {
            c_layout: Some(_), ..
        } => lir::LayoutIdentity::c_value(exact_type, target_profile, root),
        mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::FunPtr {
            ..
        }) => lir::LayoutIdentity::native_function_pointer(exact_type, target_profile, root),
        mir::StructRepresentation::Declared { c_layout: None, .. }
        | mir::StructRepresentation::Intrinsic(_) => {
            lir::LayoutIdentity::managed_value(exact_type, target_profile, root)
        }
    }
    .expect("validated exact type and target must derive a layout identity")
}

fn managed_value_layout_identity(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    ty: &mir::Type,
) -> lir::LayoutIdentity {
    lir::LayoutIdentity::managed_value(
        exact_type_record(module, ty).id(),
        context.target_profile(),
        identity_roots.for_type(ty),
    )
    .expect("validated exact type and target must derive a layout identity")
}

fn managed_object_layout_identity(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    ty: &mir::Type,
) -> lir::LayoutIdentity {
    lir::LayoutIdentity::managed_object(
        exact_type_record(module, ty).id(),
        context.target_profile(),
        identity_roots.for_type(ty),
    )
    .expect("validated exact type and target must derive a layout identity")
}
