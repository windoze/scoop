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

/// The global symbol of a TypeDescriptor. Callers supply either a fixed
/// runtime ABI name or an already encoded typed identity; source-facing
/// nominal display names never enter this helper.
pub(crate) fn td_symbol(identity: &str) -> String {
    format!("scoop_td_{identity}")
}

#[derive(Default)]
pub(crate) struct TypeDescriptorRefs {
    classes: HashMap<mir::ClassId, lir::TypeDescriptorRef>,
    interfaces: HashMap<mir::InterfaceId, lir::TypeDescriptorRef>,
    function_types: HashMap<mir::FunctionTypeId, lir::TypeDescriptorRef>,
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
            mir::Type::Function(id) => self.function_types[id],
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
            mir::Type::Any => unreachable!("Any has no referenceable TypeDescriptor"),
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
        let encoded = mir::encode_type(module, &ty)
            .expect("MIR interface applications have source type encodings");
        let runtime_type = runtime_type(module, &ty);
        let root = identity_roots.for_type(&ty);
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root)
            .expect("validated interface exact type must derive descriptor identities");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("validated interface exact type must derive a vtable identity");
        let id = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&encoded),
            identity,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.interfaces
            .insert(interface, lir::TypeDescriptorRef::Local(id));
    }
    let mut function_types = module
        .meta
        .source_exact_types
        .iter()
        .filter_map(|identity| match identity.ty() {
            mir::Type::Function(id) => Some((*id, identity.identity_record().id())),
            _ => None,
        })
        .collect::<Vec<_>>();
    function_types.sort_by_key(|(_, identity)| *identity);
    for (id, exact_type) in function_types {
        let name = format!(
            "function${}",
            mir::encode_type(module, &mir::Type::Function(id))
                .expect("MIR function types have a compact-v2 source encoding")
        );
        let runtime_type = lir::RuntimeTypeMappingRecord::new(exact_type)
            .expect("validated function exact type must derive a nonzero runtime id");
        let root = identity_roots.for_type(&mir::Type::Function(id));
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root)
            .expect("validated function exact type must derive descriptor identities");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("validated function exact type must derive a vtable identity");
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            symbol: td_symbol(&name),
            name,
            identity,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.function_types
            .insert(id, lir::TypeDescriptorRef::Local(descriptor));
    }
    let mut string = None;
    for id in class_order(module) {
        let def = &module.classes[id];
        let is_string = matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        );
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
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root)
            .expect("validated closure exact type must derive descriptor identities");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("validated closure exact type must derive a vtable identity");
        let itables = def
            .bridges
            .iter()
            .map(|bridge| {
                lir::ItableRecord::new(
                    &identity,
                    exact_type_record(module, &mir::Type::Function(bridge.target)).id(),
                    refs.function_types[&bridge.target],
                    vec![dispatch_entry(
                        &mir::TableSlot::Function(bridge.function),
                        local_functions,
                    )],
                )
                .expect("validated closure and function exact types must derive an itable identity")
            })
            .collect();
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(def.link_stem.as_str()),
            identity,
            size,
            align,
            scan: lir::TypeDescriptorScan::Fixed(scan),
            parent: Some(refs.function_types[&def.function_type]),
            vtable,
            itables,
        });
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    refs.boxed = module
        .meta
        .boxed_types
        .iter()
        .map(|boxed| (boxed.payload().clone(), refs.classes[&boxed.class()]))
        .collect();
    let string = string.expect("LocalConcreteHir supplies the typed intrinsic String descriptor");
    (descriptors, refs, lir::WellKnownTypeDescriptors { string })
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
    let identity = lir::TypeDescriptorIdentity::new(runtime_type, root)
        .expect("validated class exact type must derive descriptor identities");
    let (size, align, scan) = class_layout(context, module, enums, def);
    let scan = match &def.representation {
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. },
        ) => lir::TypeDescriptorScan::ArrayElement { stride: size, scan },
        _ => lir::TypeDescriptorScan::Fixed(scan),
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
        .map(|record| {
            lir::ItableRecord::new(
                &identity,
                exact_type_record(module, &mir::Type::Interface(record.interface)).id(),
                refs.interfaces[&record.interface],
                record
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, local_functions))
                    .collect(),
            )
            .expect("validated class and interface exact types must derive an itable identity")
        })
        .collect();
    lir::TypeDescriptor {
        name: def.name.clone(),
        symbol: if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            lir::STRING_TD_SYMBOL.to_string()
        } else {
            let encoded = mir::encode_type(module, &mir::Type::Class(id))
                .expect("MIR class applications have source type encodings");
            td_symbol(&encoded)
        },
        identity,
        size,
        align,
        scan,
        parent: def.base_class().map(|base| refs.classes[&base]),
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
                exact_type_record(module, &mir::Type::Class(class_id)).id(),
                context.target_profile(),
                identity_roots.for_type(&mir::Type::Class(class_id)),
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
