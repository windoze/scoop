mod arrays;
mod shapes;
pub(crate) use arrays::array_types;
use shapes::*;

use super::*;

/// Class ids ordered base-before-derived (single inheritance: depth
/// in the base chain; ties keep declaration order).
pub(crate) fn class_order(module: &mir::Module) -> StorageResult<Vec<mir::ClassId>> {
    let mut order = Vec::with_capacity(module.classes.len());
    for (id, _) in module.classes.iter() {
        let mut seen = std::collections::HashSet::new();
        let mut current = Some(id);
        while let Some(class) = current {
            if !seen.insert(class) {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "class base cycle",
                ));
            }
            current = module.classes[class].base_class();
        }
        order.push((seen.len(), id));
    }
    order.sort_by_key(|(depth, _)| *depth);
    Ok(order.into_iter().map(|(_, id)| id).collect())
}

#[derive(Default)]
pub(crate) struct TypeDescriptorRefs {
    classes: HashMap<mir::ClassId, lir::TypeDescriptorRef>,
    interfaces: HashMap<mir::InterfaceId, lir::TypeDescriptorRef>,
    closures: HashMap<mir::ClosureClassId, lir::TypeDescriptorRef>,
    boxed: Vec<(mir::Type, lir::BoxedValueDescriptor)>,
    string: Option<lir::TypeDescriptorRef>,
}

impl TypeDescriptorRefs {
    pub(crate) fn for_boxed_type(&self, ty: &mir::Type) -> lir::BoxedValueDescriptor {
        self.boxed
            .iter()
            .find_map(|(payload, descriptor)| (payload == ty).then(|| descriptor.clone()))
            .unwrap_or_else(|| panic!("MIR did not supply a boxed descriptor for {ty:?}"))
    }

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
                .find_map(|(payload, descriptor)| {
                    (payload == ty).then(|| match descriptor {
                        lir::BoxedValueDescriptor::ZeroSized(descriptor) => descriptor.descriptor(),
                        lir::BoxedValueDescriptor::NonZero(descriptor) => descriptor.descriptor(),
                    })
                })
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
    dependencies: crate::dependency_types::DependencyTypeDescriptors,
) -> Result<
    (
        Arena<lir::TypeDescriptor>,
        TypeDescriptorRefs,
        lir::WellKnownTypeDescriptors,
    ),
    StrongLirLoweringError,
> {
    let imported = dependencies
        .source
        .iter()
        .map(|(ty, _)| ty.clone())
        .collect::<Vec<_>>();
    let mut descriptors = Arena::new();
    let mut refs = TypeDescriptorRefs {
        boxed: dependencies.boxed,
        ..TypeDescriptorRefs::default()
    };
    for (ty, reference) in dependencies.source {
        match ty {
            mir::Type::Class(id) => {
                refs.classes.insert(id, reference);
            }
            mir::Type::Interface(id) => {
                refs.interfaces.insert(id, reference);
            }
            mir::Type::String => {
                refs.string = Some(reference);
            }
            _ => unreachable!("only reference nominals have direct source descriptor imports"),
        }
    }
    for (location, reference) in dependencies.generated {
        if let mir::GeneratedExactTypeLocation::Class(id) = location {
            refs.classes.insert(id, reference);
        }
    }
    for (interface, def) in module.interfaces.iter() {
        let ty = mir::Type::Interface(interface);
        if imported.contains(&ty) || !identity_roots.materializes_type(&ty) {
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
            inline_scan: lir::TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.interfaces
            .insert(interface, lir::TypeDescriptorRef::Local(id));
    }
    for id in class_order(module)? {
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
        if imported.contains(&descriptor_type) || refs.classes.contains_key(&id) {
            continue;
        }
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
        )?;
        let descriptor = lir::TypeDescriptorRef::Local(descriptors.alloc(descriptor));
        assert!(refs.classes.insert(id, descriptor).is_none());
        if is_string {
            refs.string = Some(descriptor);
        }
    }
    for (closure, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def)?;
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
            .map_err(StorageLoweringError::from)?,
            inline_scan: lir::TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    for root in identity_roots.source_nominal_shapes() {
        if imported.contains(root.ty()) {
            continue;
        }
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
        )?);
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
        )?);
    }
    for boxed in &module.meta.boxed_types {
        if !identity_roots.materializes_type(&mir::Type::Class(boxed.class())) {
            continue;
        }
        let Some(descriptor) = refs.classes.get(&boxed.class()).copied() else {
            continue;
        };
        let lir::TypeDescriptorRef::Local(descriptor) = descriptor else {
            return Err(StorageLoweringError::InvalidRepresentation(
                "local MIR boxed helpers require local descriptors",
            )
            .into());
        };
        let proof = lir::BoxedValueDescriptor::from_local(
            &descriptors,
            descriptor,
            exact_type_record(module, boxed.payload()).id(),
            lir_type(boxed.payload()),
        )
        .map_err(StorageLoweringError::from)?;
        refs.boxed.push((boxed.payload().clone(), proof));
    }
    let string = refs
        .string
        .ok_or(StrongLirLoweringError::MissingRuntimeStringDescriptor {
            producer: module.cone,
        })?;
    Ok((descriptors, refs, lir::WellKnownTypeDescriptors { string }))
}
