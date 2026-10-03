use super::*;
mod objects;
mod shapes;
mod values;

pub(crate) use objects::*;
pub(crate) use shapes::*;
pub(crate) use values::*;

/// Persistent layouts selected by the sealed materialization plan and the
/// actual inline payloads required by boxes, arrays, and managed globals.
pub(crate) fn layouts(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    emit_runtime_string: bool,
) -> StorageResult<Arena<lir::Layout>> {
    let mut layouts = Arena::new();
    for (id, def) in module.structs.iter() {
        let ty = def.physical_type(id);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        let identity = struct_layout_identity(context, identity_roots, module, &ty, def);
        let needs_managed_value = identity.layout_record().key().representation()
            != scoop_identity::RepresentationRole::ManagedValue;
        layouts.alloc(struct_layout(context, module, enums, identity, def)?);
        if needs_managed_value {
            layouts.alloc(struct_layout(
                context,
                module,
                enums,
                managed_value_layout_identity(context, identity_roots, module, &ty),
                def,
            )?);
        }
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
    for (id, def) in module.interfaces.iter() {
        let ty = mir::Type::Interface(id);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        layouts.alloc(managed_reference_value_layout(
            context,
            managed_value_layout_identity(context, identity_roots, module, &ty),
            &def.name,
        ));
    }
    for (id, def) in module.classes.iter() {
        let ty = if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            mir::Type::String
        } else {
            mir::Type::Class(id)
        };
        if matches!(ty, mir::Type::String) && !emit_runtime_string {
            continue;
        }
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        layouts.alloc(managed_reference_value_layout(
            context,
            managed_value_layout_identity(context, identity_roots, module, &ty),
            &def.name,
        ));
        match def.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
                layouts.alloc(class_definition_layout(
                    context,
                    module,
                    enums,
                    managed_object_layout_identity(context, identity_roots, module, &ty),
                    def,
                )?);
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
                )?);
            }
        }
    }
    for (id, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def)?;
        let exact =
            generated_exact_type_record(module, mir::GeneratedExactTypeLocation::Closure(id)).id();
        let root = identity_roots.for_generated(mir::GeneratedExactTypeLocation::Closure(id));
        layouts.alloc(managed_reference_value_layout(
            context,
            lir::LayoutIdentity::managed_value(exact, context.target_profile(), root.clone())
                .expect("validated exact type and target must derive a layout identity"),
            &def.name,
        ));
        layouts.alloc(lir::Layout {
            identity: lir::LayoutIdentity::managed_object(exact, context.target_profile(), root)
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
    for root in identity_roots.source_nominal_shapes() {
        if matches!(root.ty(), mir::Type::String) && !emit_runtime_string {
            continue;
        }
        if layouts.iter().any(|(_, layout)| {
            layout.identity.layout_record().key().exact_type() == root.exact()
                && layout.identity.layout_record().key().representation()
                    == scoop_identity::RepresentationRole::ManagedValue
        }) {
            continue;
        }
        let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
        let (size, align) = size_align(context, module, &enum_shape, root.ty())?;
        layouts.alloc(lir::Layout {
            identity: managed_value_layout_identity(context, identity_roots, module, root.ty()),
            name: mir::type_name(module, root.ty()),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Plain {
                scan: ref_scan(context, module, enums, root.ty(), 0)?,
            },
        });
    }
    let inline_payloads = module
        .meta
        .boxed_types
        .iter()
        .filter(|boxed| identity_roots.materializes_type(&mir::Type::Class(boxed.class())))
        .map(|boxed| boxed.payload())
        .chain(module.classes.iter().filter_map(|(id, class)| {
            if !identity_roots.materializes_type(&mir::Type::Class(id)) {
                return None;
            }
            match &class.representation {
                mir::ClassRepresentation::Intrinsic(
                    mir::IntrinsicTypeRepresentation::Array { element }
                    | mir::IntrinsicTypeRepresentation::MutableArray { element },
                ) => Some(element),
                _ => None,
            }
        }))
        .chain(module.globals.iter().filter_map(|(_, global)| {
            matches!(global.storage, mir::GlobalStorage::Managed { .. }).then_some(&global.ty)
        }));
    for payload in inline_payloads {
        let Some(source) = module.meta.source_exact_types.get(payload) else {
            continue;
        };
        if source.owner() != mir::SourceExactTypeOwner::Structural
            || layouts.iter().any(|(_, layout)| {
                layout.identity.layout_record().key().exact_type() == source.identity_record().id()
                    && layout.identity.layout_record().key().representation()
                        == scoop_identity::RepresentationRole::ManagedValue
            })
        {
            continue;
        }
        let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
        let (size, align) = size_align(context, module, &enum_shape, payload)?;
        layouts.alloc(lir::Layout {
            identity: managed_value_layout_identity(context, identity_roots, module, payload),
            name: mir::type_name(module, payload),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Plain {
                scan: ref_scan(context, module, enums, payload, 0)?,
            },
        });
    }
    Ok(layouts)
}

fn managed_reference_value_layout(
    context: &LoweringContext,
    identity: lir::LayoutIdentity,
    name: &str,
) -> lir::Layout {
    let pointer = context.pointer_layout(lir::PointerKind::Managed);
    lir::Layout {
        identity,
        name: format!("{name} value"),
        size: pointer.size,
        align: pointer.align,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: lir::LayoutKind::Plain {
            scan: lir::RefScan::References(vec![0]),
        },
    }
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
