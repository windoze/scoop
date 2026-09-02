use super::*;

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
                export::ClassRepresentation::Declared(_),
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::ClassRepresentation::Declared {
                constructor: Vec::new(),
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
        let id = self.classes.alloc(concrete::ClassDef {
            origin: concrete::ClassOriginId::from_raw(source_id.into_raw().into_u32()),
            modifier: source.modifier,
            name: self.instance_name(&source.name, &arguments),
            type_arguments: arguments.clone(),
            representation,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        self.class_by_key.insert(key, id);
        self.class_source.insert(id, source_id);
        let ty = match self.classes[id].representation {
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::String,
                ..
            } => self.intern_type(concrete::TypeKind::String, false),
            concrete::ClassRepresentation::Declared { .. }
            | concrete::ClassRepresentation::Intrinsic {
                application:
                    concrete::IntrinsicTypeRepresentation::Array { .. }
                    | concrete::IntrinsicTypeRepresentation::MutableArray { .. },
                ..
            } => self.intern_type(concrete::TypeKind::Class(id), false),
            concrete::ClassRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        self.class_type.insert(id, ty);

        // Reserve the hidden callable identity before lowering constructor
        // expressions. A base-delegation expression may recursively mention
        // this already-interned class, and must still see the same target.
        let constructor_id = if source.modifier != export::ClassModifier::Abstract
            && matches!(
                &source.representation,
                export::ClassRepresentation::Declared(_)
            ) {
            let raw = self.class_constructor_slots.len() as u32;
            self.class_constructor_slots.push(None);
            let constructor = concrete::ClassConstructorId::from_raw(raw.into());
            assert!(
                self.class_constructor_by_class
                    .insert(id, constructor)
                    .is_none()
            );
            Some(constructor)
        } else {
            None
        };

        let constructor: Vec<concrete::ConstructorField> = source
            .semantic_constructor()
            .iter()
            .map(|field| concrete::ConstructorField {
                parameter: concrete::ConstructorParamId::from_raw(field.parameter.into_raw()),
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &arguments),
                mutable: field.mutable,
            })
            .collect();
        let constructor_params = constructor
            .iter()
            .map(|field: &concrete::ConstructorField| field.ty)
            .collect();
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Class(id));
        let interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, &arguments);
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        let base_class = source.base_class.map(|(base, args)| {
            let base = self.lower_type(base, &arguments);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("class bases concretize to class identities")
            };
            let args = args
                .iter()
                .map(|argument| self.lower_expr(argument, &arguments, &[]))
                .collect();
            (base, args)
        });
        self.classes[id].interfaces = interfaces;
        self.classes[id].interface_implementations = interface_implementations;
        match &mut self.classes[id].representation {
            concrete::ClassRepresentation::Declared {
                constructor: concrete_constructor,
                base_class: concrete_base,
            } => {
                *concrete_constructor = constructor;
                *concrete_base = base_class;
            }
            concrete::ClassRepresentation::Intrinsic { .. } => {
                debug_assert!(constructor.is_empty() && base_class.is_none());
            }
        }
        self.classes[id].methods = methods;
        if let Some(constructor_id) = constructor_id {
            let slot = constructor_id.into_raw().into_u32() as usize;
            assert!(
                self.class_constructor_slots[slot]
                    .replace(concrete::ClassConstructor {
                        class: id,
                        params: constructor_params,
                        return_type: ty,
                    })
                    .is_none()
            );
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
                let interface =
                    self.lower_interface_application(implementation.interface, substitution);
                let methods = implementation
                    .methods
                    .into_iter()
                    .map(|method| {
                        let slot = self.interface_slot_by_source[&(interface, method.member)];
                        let target = match method.target {
                            export::InterfaceImplementationTarget::Method(application) => {
                                let concrete::Callable::Function(function) =
                                    self.lower_method_application(application, substitution);
                                concrete::InterfaceImplementationTarget::Method(function)
                            }
                            export::InterfaceImplementationTarget::Subclass => {
                                let source = self.source.interface_methods[method.member].function;
                                let declaration = self.request_method(
                                    source,
                                    concrete::MethodOwner::Interface(interface),
                                    MethodRequest::Plain,
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

    pub(super) fn lower_extern_functions(&mut self) {
        for (source_id, source) in self.source.extern_functions.iter() {
            let params = source
                .params
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let return_type = self.lower_type(source.return_type, &[]);
            let id = self.extern_functions.alloc(concrete::ExternFunction {
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
            let storage = self.lower_global_storage(&source.storage);
            let id = self.globals.alloc(concrete::Global {
                name: source.name.clone(),
                ty,
                mutable: source.mutable,
                storage,
                span: source.span,
            });
            self.global_map.insert(source_id, id);
        }
    }

    pub(super) fn lower_global_storage(
        &mut self,
        storage: &export::GlobalStorage,
    ) -> concrete::GlobalStorage {
        match storage {
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
                library: library.clone(),
                native_symbol: native_symbol.clone(),
                thread_local: *thread_local,
            },
        }
    }

    pub(super) fn lower_constant(
        &mut self,
        value: &export::ConstantValue,
    ) -> concrete::ConstantValue {
        match value {
            export::ConstantValue::Int(value) => concrete::ConstantValue::Int(*value),
            export::ConstantValue::Bool(value) => concrete::ConstantValue::Bool(*value),
            export::ConstantValue::NullPtr => concrete::ConstantValue::NullPtr,
            export::ConstantValue::NullFunPtr => concrete::ConstantValue::NullFunPtr,
            export::ConstantValue::Struct {
                application,
                fields,
            } => {
                let struct_id = self.lower_struct_application(*application, &[]);
                concrete::ConstantValue::Struct {
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
