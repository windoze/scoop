use super::*;

/// Class ids ordered base-before-derived (single inheritance: depth
/// in the base chain; ties keep declaration order).
pub(crate) fn class_order(module: &mir::Module) -> Vec<mir::ClassId> {
    fn depth(module: &mir::Module, id: mir::ClassId) -> usize {
        match module.classes[id].base_class() {
            Some(base) => depth(module, base) + 1,
            None => 0,
        }
    }
    let mut order: Vec<mir::ClassId> = module.classes.iter().map(|(id, _)| id).collect();
    order.sort_by_key(|&id| depth(module, id));
    order
}

#[derive(Default)]
pub(crate) struct TypeDescriptorRefs {
    classes: HashMap<mir::ClassId, lir::TypeDescriptorRef>,
    interfaces: HashMap<mir::InterfaceId, lir::TypeDescriptorRef>,
    closures: HashMap<mir::ClosureClassId, lir::TypeDescriptorRef>,
    boxed: Vec<(mir::Type, lir::TypeDescriptorRef)>,
    string: Option<lir::TypeDescriptorRef>,
}

impl TypeDescriptorRefs {
    pub(crate) fn for_closure(&self, id: mir::ClosureClassId) -> lir::TypeDescriptorRef {
        self.closures[&id]
    }

    pub(crate) fn for_type(&self, ty: &mir::Type) -> lir::TypeDescriptorRef {
        match ty {
            mir::Type::Class(id) => self.classes[id],
            mir::Type::Interface(id) => self.interfaces[id],
            mir::Type::String => self.string.expect("typed String descriptor"),
            mir::Type::Struct(_)
            | mir::Type::Enum(..)
            | mir::Type::Tuple(_)
            | mir::Type::Integer(_)
            | mir::Type::MachineScalar(_)
            | mir::Type::Boolean
            | mir::Type::Unit
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_) => self
                .boxed
                .iter()
                .find_map(|(payload, descriptor)| (payload == ty).then_some(*descriptor))
                .unwrap_or_else(|| panic!("MIR did not supply a boxed descriptor for {ty:?}")),
            mir::Type::Function(_) | mir::Type::Any => {
                unreachable!("the strong capability gate rejects this TypeDescriptor request")
            }
        }
    }
}

pub(crate) fn dispatch_entry(
    slot: &mir::TableSlot,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> lir::DispatchEntry {
    lir::DispatchEntry {
        callable: match slot {
            mir::TableSlot::Function(id) => {
                lir::CallableRef::Local(local_functions[id].declaration())
            }
            mir::TableSlot::Runtime(function) => {
                lir::CallableRef::Runtime(lower_runtime_function(*function))
            }
        },
    }
}

/// Build the complete local TypeDescriptor graph before lowering any body.
/// Parent, interface, dispatch and operand references can therefore use typed
/// ids directly; symbols remain emission attributes only.
pub(crate) fn type_descriptors(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> (
    Arena<lir::TypeDescriptor>,
    TypeDescriptorRefs,
    lir::WellKnownTypeDescriptors,
) {
    let mut descriptors = Arena::new();
    let mut refs = TypeDescriptorRefs::default();
    for (interface, def) in module.interfaces.iter() {
        let ty = mir::Type::Interface(interface);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        let runtime_type = runtime_type(module, &ty);
        let root = identity_roots.for_type(&ty);
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
            .expect("validated interface exact type must derive descriptor identities");
        let instance_layout = lir::LayoutIdentity::managed_object(
            runtime_type.exact_type(),
            context.target_profile(),
            root,
        )
        .expect("validated interface exact type must derive its instance layout identity");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("validated interface exact type must derive a vtable identity");
        let id = descriptors.alloc(lir::TypeDescriptor {
            diagnostic_name: def.name.clone(),
            identity,
            instance_layout,
            instance_shape: lir::TypeInstanceShapeV1::abstract_ref(),
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.interfaces
            .insert(interface, lir::TypeDescriptorRef::Local(id));
    }
    let mut string = None;
    for id in class_order(module) {
        let def = &module.classes[id];
        let is_string = matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        );
        let descriptor_type = if is_string {
            mir::Type::String
        } else {
            mir::Type::Class(id)
        };
        if !identity_roots.materializes_type(&descriptor_type) {
            continue;
        }
        let descriptor = class_type_descriptor(
            context,
            identity_roots,
            module,
            enums,
            id,
            &refs,
            local_functions,
        );
        let descriptor = lir::TypeDescriptorRef::Local(descriptors.alloc(descriptor));
        assert!(refs.classes.insert(id, descriptor).is_none());
        if is_string {
            assert!(
                string.replace(descriptor).is_none(),
                "one typed String TypeDescriptor"
            );
            refs.string = Some(descriptor);
        }
    }
    for (closure, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def);
        let runtime_type =
            generated_runtime_type(module, mir::GeneratedExactTypeLocation::Closure(closure));
        let root = identity_roots.for_generated(mir::GeneratedExactTypeLocation::Closure(closure));
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
            .expect("validated closure exact type must derive descriptor identities");
        let instance_layout = lir::LayoutIdentity::managed_object(
            runtime_type.exact_type(),
            context.target_profile(),
            root,
        )
        .expect("validated closure exact type must derive its instance layout identity");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("validated closure exact type must derive a vtable identity");
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            diagnostic_name: def.name.clone(),
            identity,
            instance_layout,
            instance_shape: lir::TypeInstanceShapeV1::fixed_object(
                context.target_profile(),
                size,
                align,
                scan,
            )
            .expect("validated closure layout must form a managed instance shape"),
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    for root in identity_roots.source_nominal_shapes() {
        if descriptors
            .iter()
            .any(|(_, descriptor)| descriptor.identity.exact_type() == root.exact())
        {
            continue;
        }
        descriptors.alloc(value_or_abstract_type_descriptor(
            context,
            module,
            enums,
            root.ty(),
            root.exact(),
            identity_roots.for_type(root.ty()),
        ));
    }
    for root in identity_roots.generated_nominal_shapes() {
        if descriptors
            .iter()
            .any(|(_, descriptor)| descriptor.identity.exact_type() == root.exact())
        {
            continue;
        }
        let ty = match root.location() {
            mir::GeneratedExactTypeLocation::Class(id) => mir::Type::Class(id),
            mir::GeneratedExactTypeLocation::Enum(id) => {
                mir::Type::Enum(id, module.enums[id].type_arguments.clone())
            }
            mir::GeneratedExactTypeLocation::Closure(_) => {
                unreachable!("closure generated exact types have dedicated descriptors")
            }
        };
        descriptors.alloc(value_or_abstract_type_descriptor(
            context,
            module,
            enums,
            &ty,
            root.exact(),
            identity_roots.for_generated(root.location()),
        ));
    }
    refs.boxed = module
        .meta
        .boxed_types
        .iter()
        .filter_map(|boxed| {
            refs.classes
                .get(&boxed.class())
                .copied()
                .map(|descriptor| (boxed.payload().clone(), descriptor))
        })
        .collect();
    let string = string.expect("LocalConcreteHir supplies the typed intrinsic String descriptor");
    (descriptors, refs, lir::WellKnownTypeDescriptors { string })
}

fn value_or_abstract_type_descriptor(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    ty: &mir::Type,
    exact: scoop_identity::PersistentExactTypeId,
    root: lir::MaterializationRoot,
) -> lir::TypeDescriptor {
    let identity = lir::TypeDescriptorIdentity::new(
        lir::RuntimeTypeMappingRecord::new(exact)
            .expect("validated exact type must derive a nonzero runtime id"),
        root.clone(),
    )
    .expect("validated exact type must derive descriptor identities");
    let instance_layout =
        lir::LayoutIdentity::managed_object(exact, context.target_profile(), root)
            .expect("validated exact type and target must derive its instance layout identity");
    let instance_shape = if matches!(ty, mir::Type::Any) {
        lir::TypeInstanceShapeV1::abstract_ref()
    } else {
        assert!(
            !matches!(
                ty,
                mir::Type::String
                    | mir::Type::Class(_)
                    | mir::Type::Interface(_)
                    | mir::Type::Function(_)
            ),
            "reference source nominals have dedicated descriptors"
        );
        let enum_shape = |id: mir::EnumId| repr_shape(context, &enums[enum_def_id(id)].repr);
        let (size, align) = size_align(context, module, &enum_shape, ty);
        let scan = ref_scan(context, module, enums, ty, 0);
        let value = if size == 0 {
            lir::ValueStorageLayoutV1::zero_sized(align)
        } else {
            lir::ValueStorageLayoutV1::inline(size, align, scan)
        }
        .expect("validated value type must form boxed inline storage");
        lir::TypeInstanceShapeV1::boxed_value(context.target_profile(), value)
            .expect("validated value type must form a managed box instance shape")
    };
    let vtable = lir::VtableRecord::new(&identity, Vec::new())
        .expect("validated exact type must derive a vtable identity");
    lir::TypeDescriptor {
        diagnostic_name: mir::type_name(module, ty),
        identity,
        instance_layout,
        instance_shape,
        parent: None,
        vtable,
        itables: Vec::new(),
    }
}

pub(crate) fn class_type_descriptor(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    id: mir::ClassId,
    refs: &TypeDescriptorRefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> lir::TypeDescriptor {
    let def = &module.classes[id];
    let descriptor_type = if matches!(
        def.representation,
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
    ) {
        mir::Type::String
    } else {
        mir::Type::Class(id)
    };
    let runtime_type = runtime_type(module, &descriptor_type);
    let root = identity_roots.for_type(&descriptor_type);
    let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
        .expect("validated class exact type must derive descriptor identities");
    let instance_layout = lir::LayoutIdentity::managed_object(
        runtime_type.exact_type(),
        context.target_profile(),
        root,
    )
    .expect("validated class exact type must derive its instance layout identity");
    let instance_shape = match &def.representation {
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
            lir::TypeInstanceShapeV1::inline_bytes(context.target_profile())
                .expect("the target profile must form the String instance shape")
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. },
        ) => {
            let (size, align, scan) = class_layout(context, module, enums, def);
            let element = if size == 0 {
                lir::ArrayElementStorageV1::zero_sized(align)
            } else {
                lir::ArrayElementStorageV1::inline(size, align, scan)
            }
            .expect("validated array element layout must form inline storage");
            lir::TypeInstanceShapeV1::inline_array(context.target_profile(), element)
                .expect("validated array layout must form a managed instance shape")
        }
        mir::ClassRepresentation::Declared { .. } => {
            if let Some(boxed) = module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| boxed.class() == id)
            {
                let enum_shape =
                    |id: mir::EnumId| repr_shape(context, &enums[enum_def_id(id)].repr);
                let (size, align) = size_align(context, module, &enum_shape, boxed.payload());
                let scan = ref_scan(context, module, enums, boxed.payload(), 0);
                let value = if size == 0 {
                    lir::ValueStorageLayoutV1::zero_sized(align)
                } else {
                    lir::ValueStorageLayoutV1::inline(size, align, scan)
                }
                .expect("validated boxed payload must form inline storage");
                lir::TypeInstanceShapeV1::boxed_value(context.target_profile(), value)
                    .expect("validated box layout must form a managed instance shape")
            } else {
                let (size, align, scan) = class_layout(context, module, enums, def);
                lir::TypeInstanceShapeV1::fixed_object(context.target_profile(), size, align, scan)
                    .expect("validated class layout must form a managed instance shape")
            }
        }
        mir::ClassRepresentation::Intrinsic(_) => {
            unreachable!("the registry fixes intrinsic declaration targets")
        }
    };
    let vtable = lir::VtableRecord::new(
        &identity,
        def.vtable
            .iter()
            .map(|slot| dispatch_entry(slot, local_functions))
            .collect(),
    )
    .expect("validated class exact type must derive a vtable identity");
    let itables = def
        .itables
        .iter()
        .filter_map(|record| {
            let interface = refs.interfaces.get(&record.interface).copied()?;
            lir::ItableRecord::new(
                &identity,
                exact_type_record(module, &mir::Type::Interface(record.interface)).id(),
                interface,
                record
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, local_functions))
                    .collect(),
            )
            .expect("validated class and interface exact types must derive an itable identity")
            .into()
        })
        .collect();
    lir::TypeDescriptor {
        diagnostic_name: def.name.clone(),
        identity,
        instance_layout,
        instance_shape,
        parent: def
            .base_class()
            .and_then(|base| refs.classes.get(&base).copied()),
        vtable,
        itables,
    }
}

fn runtime_type(module: &mir::Module, ty: &mir::Type) -> lir::RuntimeTypeMappingRecord {
    lir::RuntimeTypeMappingRecord::new(exact_type_record(module, ty).id())
        .expect("validated exact type must derive a nonzero runtime id")
}

fn generated_runtime_type(
    module: &mir::Module,
    location: mir::GeneratedExactTypeLocation,
) -> lir::RuntimeTypeMappingRecord {
    lir::RuntimeTypeMappingRecord::new(generated_exact_type_record(module, location).id())
        .expect("validated generated exact type must derive a nonzero runtime id")
}

/// Transpose every concrete intrinsic array class application into one typed
/// LIR metadata record. The MIR class id -> LIR array id map is complete before
/// function lowering starts, so no instruction discovers metadata on demand.
pub(crate) fn array_types(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    descriptors: &TypeDescriptorRefs,
) -> (
    Arena<lir::ArrayType>,
    HashMap<mir::ClassId, lir::ArrayTypeId>,
) {
    let mut arrays = Arena::new();
    let mut ids = HashMap::new();
    for (class_id, class) in module.classes.iter() {
        let ty = mir::Type::Class(class_id);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        let (kind, element) = match &class.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Array {
                element,
            }) => (lir::ArrayKind::Immutable, element),
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => (lir::ArrayKind::Mutable, element),
            mir::ClassRepresentation::Declared { .. } | mir::ClassRepresentation::Intrinsic(_) => {
                continue;
            }
        };
        let (element_size, element_align, _) = class_layout(context, module, enums, class);
        let id = arrays.alloc(lir::ArrayType {
            identity: lir::LayoutIdentity::managed_array(
                exact_type_record(module, &ty).id(),
                context.target_profile(),
                identity_roots.for_type(&ty),
            )
            .expect("validated array exact type and target must derive layout identities"),
            kind,
            element: lir_type(element),
            element_size,
            element_align,
            type_descriptor: descriptors.classes[&class_id],
        });
        assert!(ids.insert(class_id, id).is_none());
    }
    (arrays, ids)
}
