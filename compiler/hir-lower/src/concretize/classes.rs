use super::*;

mod constructors;

impl Concretizer<'_> {
    pub(super) fn ensure_class(
        &mut self,
        source_id: export::ClassId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::ClassId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.class_by_key.get(&key) {
            return id;
        }
        let source = self.source.classes[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let representation = match (&source.representation, application) {
            (
                export::ClassRepresentation::Declared,
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            (
                export::ClassRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => concrete::ClassRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("ExportHir declaration and application representations agree"),
        };
        let id = concrete::ClassId::from_raw(
            u32::try_from(self.classes.len())
                .expect("concrete class ids fit in u32")
                .into(),
        );
        let type_kind = match &representation {
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::String,
                ..
            } => concrete::TypeKind::String,
            concrete::ClassRepresentation::Declared { .. }
            | concrete::ClassRepresentation::Intrinsic {
                application:
                    concrete::IntrinsicTypeRepresentation::Array { .. }
                    | concrete::IntrinsicTypeRepresentation::MutableArray { .. },
                ..
            } => concrete::TypeKind::Class(id),
            concrete::ClassRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        let ty = self.intern_type(type_kind, false);
        let allocated = self.classes.alloc(concrete::ClassDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            modifier: source.modifier,
            name: self.source_nominal_name(&source.name, source.owner),
            owner: self.lower_nominal_owner(source.owner),
            type_arguments: arguments.clone(),
            representation,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.class_by_key.insert(key, id);
        self.class_source.insert(id, source_id);
        self.class_type.insert(id, ty);

        if let Some(object) = self.object_by_backing_class.get(&source_id).copied() {
            self.register_object(object, id, ty);
        }

        let fields: Vec<concrete::Field> = source
            .fields
            .iter()
            .map(|field| concrete::Field {
                identity: self.source.field_identities[*field].id(),
                name: self.source.properties[self.source.class_fields[*field].property]
                    .name
                    .clone(),
                ty: self.lower_type(self.source.class_fields[*field].ty, &arguments),
            })
            .collect();
        let method_owner = self.object_by_backing_class.get(&source_id).map_or(
            concrete::MethodOwner::Class(id),
            |object| {
                concrete::MethodOwner::Object(
                    self.object_type_map[&self.source.objects[*object].object_type],
                )
            },
        );
        let methods = self.request_concrete_methods(&source.methods, method_owner);
        let interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, &arguments);
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        let base_class = source.base_class.map(|base| {
            let base = self.lower_type(base, &arguments);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("class bases concretize to class identities")
            };
            base
        });
        self.classes[id].interfaces = interfaces;
        self.classes[id].interface_implementations = interface_implementations;
        match &mut self.classes[id].representation {
            concrete::ClassRepresentation::Declared {
                fields: concrete_fields,
                base_class: concrete_base,
            } => {
                *concrete_fields = fields;
                *concrete_base = base_class;
            }
            concrete::ClassRepresentation::Intrinsic { .. } => {
                debug_assert!(fields.is_empty() && base_class.is_none());
            }
        }
        self.classes[id].methods = methods;
        if source.type_params.is_empty() {
            for &constructor in &source.constructors {
                if self.automatic_class_constructor(constructor) {
                    self.request_class_constructor(constructor, id);
                }
            }
        }
        id
    }

    pub(super) fn request_concrete_methods(
        &mut self,
        source_methods: &[export::FunctionId],
        owner: concrete::MethodOwner,
    ) -> Vec<concrete::FunctionId> {
        source_methods
            .iter()
            .copied()
            .filter_map(|method| {
                let function = &self.source.functions[method];
                if !self.automatic_method(method) {
                    return None;
                }
                match function.genericity {
                    export::FunctionGenericity::Plain
                    | export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                        Some(self.request_method(method, owner, MethodRequest::Plain))
                    }
                    export::FunctionGenericity::GenericMethod { .. } => None,
                    export::FunctionGenericity::Generic { .. } => {
                        unreachable!("nominal method lists do not contain generic functions")
                    }
                }
            })
            .collect()
    }

    pub(super) fn lower_interface_implementations(
        &mut self,
        source_implementations: &[export::InterfaceImplementation],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::InterfaceImplementation> {
        source_implementations
            .iter()
            .cloned()
            .map(|implementation| {
                let interface = self.lower_interface_type(implementation.interface, substitution);
                let methods = implementation
                    .methods
                    .into_iter()
                    .map(|method| {
                        let slot = match method.member {
                            export::InterfaceMethodReference::Local(member) => {
                                self.interface_slot_by_source[&(interface, member)]
                            }
                            export::InterfaceMethodReference::Imported { slot, .. } => {
                                let export::Type::ImportedInterface(source) =
                                    &self.source.types[implementation.interface]
                                else {
                                    unreachable!("imported slots belong to imported interfaces")
                                };
                                concrete::InterfaceMethodSlot::from_raw(
                                    u32::try_from(
                                        source
                                            .methods
                                            .iter()
                                            .position(|method| method.slot.id() == slot)
                                            .expect("conformance slot belongs to the interface"),
                                    )
                                    .expect("interface slot fits in u32"),
                                )
                            }
                        };
                        let target = match method.target {
                            export::InterfaceImplementationTarget::Method(application) => {
                                let concrete::Callable::Function(function) =
                                    self.lower_method_application(application, substitution);
                                concrete::InterfaceImplementationTarget::Method(function)
                            }
                            export::InterfaceImplementationTarget::Imported(callable) => {
                                concrete::InterfaceImplementationTarget::Imported(
                                    self.imported_dependency_callable_map[&callable],
                                )
                            }
                            export::InterfaceImplementationTarget::ImportedAbstract(callable) => {
                                concrete::InterfaceImplementationTarget::ImportedAbstract {
                                    declaration: self.imported_dependency_callable_map[&callable],
                                }
                            }
                            export::InterfaceImplementationTarget::Subclass => {
                                let export::Type::Interface(application) =
                                    self.source.types[implementation.interface]
                                else {
                                    unreachable!("local abstract slots have local declarations")
                                };
                                let export::InterfaceMethodReference::Local(member) = method.member
                                else {
                                    unreachable!(
                                        "local abstract slots reference local interface methods"
                                    )
                                };
                                let declaration = self.request_abstract_interface_member(
                                    application,
                                    member,
                                    substitution,
                                );
                                concrete::InterfaceImplementationTarget::Abstract { declaration }
                            }
                        };
                        concrete::InterfaceMethodImplementation { slot, target }
                    })
                    .collect();
                concrete::InterfaceImplementation { interface, methods }
            })
            .collect()
    }

    pub(super) fn lower_interface_type(
        &mut self,
        source: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::InterfaceId {
        let ty = self.lower_type(source, substitution);
        let concrete::TypeKind::Interface(interface) = self.types[ty].kind else {
            unreachable!("interface type lowers to an interface")
        };
        interface
    }

    pub(super) fn lower_extern_functions(&mut self) {
        for (source_id, source) in self.source.extern_functions.iter() {
            let owner = self
                .source
                .functions
                .iter()
                .find_map(|(function_id, function)| {
                    matches!(&function.kind, export::FunctionKind::Extern(id) if *id == source_id)
                        .then_some(function_id)
                })
                .expect("every extern declaration has one source function owner");
            let source_contract = self
                .source
                .source_native_contracts
                .get(export::HirSourceNativeContractOwner::Function(owner))
                .expect("every extern function has a source-native contract")
                .clone();
            let params = source
                .params
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let return_type = self.lower_type(source.return_type, &[]);
            let id = self.extern_functions.alloc(concrete::ExternFunction {
                source_contract,
                source_name: source.source_name.clone(),
                native_symbol: source.native_symbol.clone(),
                library: source.library.clone(),
                abi: source.abi,
                calling_convention: source.calling_convention,
                gc_effect: source.gc_effect,
                safety: source.safety,
                params,
                return_type,
            });
            self.extern_map.insert(source_id, id);
        }
    }

    pub(super) fn lower_globals(&mut self) {
        // Allocate ids first because expressions in function bodies may refer
        // to any global regardless of declaration order.
        for (source_id, source) in self.source.globals.iter() {
            let ty = self.lower_type(source.ty, &[]);
            let storage = self.lower_global_storage(source_id, &source.storage);
            let storage_owner = self.property_storage_owner(source_id, source);
            let id = self.globals.alloc(concrete::Global {
                name: source.name.clone(),
                storage_owner,
                ty,
                mutable: source.mutable,
                storage,
                span: source.span,
            });
            self.global_map.insert(source_id, id);
        }
    }

    fn property_storage_owner(
        &self,
        global_id: export::GlobalId,
        global: &export::Global,
    ) -> concrete::PropertyStorageOwner {
        let property = &self.source.properties[global.property];
        let owner = self.source.property_identities[global.property].property_owner();
        match &property.representation {
            export::PropertyRepresentation::Stored(export::StoredProperty {
                backing: export::PropertyBacking::TopLevelGlobal { storage, .. },
            }) if *storage == global_id => concrete::PropertyStorageOwner::Backing(owner),
            export::PropertyRepresentation::Delegated { storage }
                if matches!(
                    self.source.delegate_storages[*storage].location,
                    export::DelegateStorageLocation::ManagedGlobal(id) if id == global_id
                ) =>
            {
                concrete::PropertyStorageOwner::Delegate(owner)
            }
            export::PropertyRepresentation::NativeStorage { storage } if *storage == global_id => {
                concrete::PropertyStorageOwner::Backing(owner)
            }
            export::PropertyRepresentation::Stored(_)
            | export::PropertyRepresentation::AccessorOnly
            | export::PropertyRepresentation::Delegated { .. }
            | export::PropertyRepresentation::Const { .. }
            | export::PropertyRepresentation::NativeStorage { .. } => {
                unreachable!(
                    "validated Export HIR binds every global to its physical property role"
                )
            }
        }
    }

    pub(super) fn lower_global_storage(
        &mut self,
        global: export::GlobalId,
        storage: &export::GlobalStorage,
    ) -> concrete::GlobalStorage {
        match storage {
            export::GlobalStorage::Managed { state } => concrete::GlobalStorage::Managed {
                state: match state {
                    export::HirStaticInitialState::EncodedStaticValue { payload } => {
                        concrete::HirStaticInitialState::EncodedStaticValue {
                            payload: self.lower_constant(payload),
                        }
                    }
                    export::HirStaticInitialState::ZeroedForRuntimeUnit { unit } => {
                        concrete::HirStaticInitialState::ZeroedForRuntimeUnit {
                            unit: self.request_initialization_unit(*unit),
                        }
                    }
                },
            },
            export::GlobalStorage::Local {
                thread_local,
                initializer,
            } => concrete::GlobalStorage::Local {
                thread_local: *thread_local,
                initializer: self.lower_constant(initializer),
            },
            export::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => concrete::GlobalStorage::Extern {
                source_contract: Box::new(
                    self.source
                        .source_native_contracts
                        .get(export::HirSourceNativeContractOwner::Global(global))
                        .expect("every extern global has a source-native contract")
                        .clone(),
                ),
                library: library.clone(),
                native_symbol: native_symbol.clone(),
                thread_local: *thread_local,
            },
        }
    }

    pub(super) fn lower_constant(
        &mut self,
        value: &export::HirConstantImage,
    ) -> concrete::HirConstantImage {
        match value {
            export::HirConstantImage::ImportedStruct { ty, fields } => {
                let ty = self.lower_type(*ty, &[]);
                let concrete::TypeKind::Struct(struct_id) = self.types[ty].kind else {
                    unreachable!("an imported struct constant retains its struct type")
                };
                concrete::HirConstantImage::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
            export::HirConstantImage::Integer(value) => concrete::HirConstantImage::Integer(*value),
            export::HirConstantImage::Boolean(value) => concrete::HirConstantImage::Boolean(*value),
            export::HirConstantImage::String(value) => {
                concrete::HirConstantImage::String(value.clone())
            }
            export::HirConstantImage::NullPointer(kind) => {
                concrete::HirConstantImage::NullPointer(match kind {
                    export::HirPointerNullKind::Raw => concrete::HirPointerNullKind::Raw,
                    export::HirPointerNullKind::Code => concrete::HirPointerNullKind::Code,
                })
            }
            export::HirConstantImage::EnumUnit { variant } => {
                concrete::HirConstantImage::EnumUnit {
                    variant: self.lower_applied_enum_variant_ref(*variant, &[]),
                }
            }
            export::HirConstantImage::Struct {
                application,
                fields,
            } => {
                let struct_id = self.lower_struct_application(*application, &[]);
                concrete::HirConstantImage::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
        }
    }
}
