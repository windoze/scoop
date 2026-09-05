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

/// The global symbol of a type's TypeDescriptor (`scoop_td_<name>`,
/// runtime spec 2.2).
pub(crate) fn td_symbol(name: &str) -> String {
    format!("scoop_td_{name}")
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

const FIRST_GENERATED_TD_TYPE_ID: u64 = 2;

/// Build the complete local TypeDescriptor graph before lowering any body.
/// Parent, interface, dispatch and operand references can therefore use typed
/// ids directly; symbols remain emission attributes only.
pub(crate) fn type_descriptors(
    context: &LoweringContext,
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
    let mut next_type_id = FIRST_GENERATED_TD_TYPE_ID;
    for (interface, def) in module.interfaces.iter() {
        let id = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            runtime_type_id: next_type_id,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        next_type_id += 1;
        refs.interfaces
            .insert(interface, lir::TypeDescriptorRef::Local(id));
    }
    for (id, _) in module.function_types.iter() {
        let name = format!(
            "function${}",
            mir::encode_type(module, &mir::Type::Function(id))
                .expect("MIR function types have a compact-v2 source encoding")
        );
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            symbol: td_symbol(&name),
            name,
            runtime_type_id: next_type_id,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        next_type_id += 1;
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
        let runtime_type_id = if is_string {
            1
        } else {
            let assigned = next_type_id;
            next_type_id += 1;
            assigned
        };
        let descriptor = class_type_descriptor(
            context,
            module,
            enums,
            id,
            runtime_type_id,
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
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            runtime_type_id: next_type_id,
            size,
            align,
            scan: lir::TypeDescriptorScan::Fixed(scan),
            parent: Some(refs.function_types[&def.function_type]),
            vtable: Vec::new(),
            itables: def
                .bridges
                .iter()
                .map(|bridge| lir::ItableRecord {
                    interface: refs.function_types[&bridge.target],
                    slots: vec![dispatch_entry(
                        &mir::TableSlot::Function(bridge.function),
                        local_functions,
                    )],
                })
                .collect(),
        });
        next_type_id += 1;
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    refs.boxed = module
        .meta
        .boxed_types
        .iter()
        .map(|boxed| (boxed.payload.clone(), refs.classes[&boxed.class]))
        .collect();
    let string = string.expect("LocalConcreteHir supplies the typed intrinsic String descriptor");
    (descriptors, refs, lir::WellKnownTypeDescriptors { string })
}

pub(crate) fn class_type_descriptor(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    id: mir::ClassId,
    runtime_type_id: u64,
    refs: &TypeDescriptorRefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> lir::TypeDescriptor {
    let def = &module.classes[id];
    let (size, align, scan) = class_layout(context, module, enums, def);
    let scan = match &def.representation {
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. },
        ) => lir::TypeDescriptorScan::ArrayElement { stride: size, scan },
        _ => lir::TypeDescriptorScan::Fixed(scan),
    };
    lir::TypeDescriptor {
        name: def.name.clone(),
        symbol: if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            lir::STRING_TD_SYMBOL.to_string()
        } else {
            td_symbol(&def.name)
        },
        runtime_type_id,
        size,
        align,
        scan,
        parent: def.base_class().map(|base| refs.classes[&base]),
        vtable: def
            .vtable
            .iter()
            .map(|slot| dispatch_entry(slot, local_functions))
            .collect(),
        itables: def
            .itables
            .iter()
            .map(|record| lir::ItableRecord {
                interface: refs.interfaces[&record.interface],
                slots: record
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, local_functions))
                    .collect(),
            })
            .collect(),
    }
}

/// Transpose every concrete intrinsic array class application into one typed
/// LIR metadata record. The MIR class id -> LIR array id map is complete before
/// function lowering starts, so no instruction discovers metadata on demand.
pub(crate) fn array_types(
    context: &LoweringContext,
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
