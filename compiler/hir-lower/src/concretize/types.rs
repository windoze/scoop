use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_integer_type(
        &mut self,
        kind: export::IntegerKind,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        match self.core {
            export::CoreProtocols::Defined(protocols) => {
                let owner = protocols.fundamental_types.integers.owner(kind);
                let source_type = self.source.struct_applications
                    [self.source.structs[owner].self_application]
                    .canonical_type;
                self.lower_type(source_type, substitution)
            }
            export::CoreProtocols::Imported(_) => {
                self.intern_type(concrete::TypeKind::Integer(kind), true)
            }
        }
    }

    pub(super) fn lower_struct_application(
        &mut self,
        source: export::StructApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::StructId {
        let application = self.source.struct_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        let representation = match application.representation {
            export::StructApplicationRepresentation::Declared => {
                ConcreteApplicationRepresentation::Declared
            }
            export::StructApplicationRepresentation::Intrinsic(representation) => {
                ConcreteApplicationRepresentation::Intrinsic(
                    self.lower_intrinsic_type_representation(representation, substitution),
                )
            }
        };
        self.ensure_struct(
            self.source
                .nominal_identities
                .struct_id(application.template)
                .expect("an application retains its declaration"),
            arguments,
            representation,
        )
    }

    pub(super) fn lower_enum_application(
        &mut self,
        source: export::EnumApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumId {
        let application = self.source.enum_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        self.ensure_enum(
            self.source
                .nominal_identities
                .enum_id(application.template)
                .expect("an application retains its declaration"),
            arguments,
        )
    }

    pub(super) fn lower_class_application(
        &mut self,
        source: export::ClassApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassId {
        let application = self.source.class_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        let representation = match application.representation {
            export::ClassApplicationRepresentation::Declared => {
                ConcreteApplicationRepresentation::Declared
            }
            export::ClassApplicationRepresentation::Intrinsic(representation) => {
                ConcreteApplicationRepresentation::Intrinsic(
                    self.lower_intrinsic_type_representation(representation, substitution),
                )
            }
        };
        self.ensure_class(
            self.source
                .nominal_identities
                .class_id(application.template)
                .expect("an application retains its declaration"),
            arguments,
            representation,
        )
    }

    pub(super) fn lower_intrinsic_type_representation(
        &mut self,
        representation: export::IntrinsicTypeRepresentation,
        substitution: &[concrete::TypeId],
    ) -> concrete::IntrinsicTypeRepresentation {
        match representation {
            export::IntrinsicTypeRepresentation::Integer(kind) => {
                concrete::IntrinsicTypeRepresentation::Integer(kind)
            }
            export::IntrinsicTypeRepresentation::Boolean => {
                concrete::IntrinsicTypeRepresentation::Boolean
            }
            export::IntrinsicTypeRepresentation::String => {
                concrete::IntrinsicTypeRepresentation::String
            }
            export::IntrinsicTypeRepresentation::Array { element } => {
                concrete::IntrinsicTypeRepresentation::Array {
                    element: self.lower_type(element, substitution),
                }
            }
            export::IntrinsicTypeRepresentation::MutableArray { element } => {
                concrete::IntrinsicTypeRepresentation::MutableArray {
                    element: self.lower_type(element, substitution),
                }
            }
            export::IntrinsicTypeRepresentation::Ptr { pointee } => {
                concrete::IntrinsicTypeRepresentation::Ptr {
                    pointee: self.lower_type(pointee, substitution),
                }
            }
            export::IntrinsicTypeRepresentation::FunPtr { function } => {
                let function = self.lower_type(function, substitution);
                let concrete::TypeKind::Function(signature) = self.types[function].kind else {
                    panic!("validated FunPtr arguments remain function types after substitution")
                };
                concrete::IntrinsicTypeRepresentation::FunPtr { signature }
            }
        }
    }

    pub(super) fn lower_interface_application(
        &mut self,
        source: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::InterfaceId {
        let application = self.source.interface_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        self.ensure_interface(
            self.source
                .nominal_identities
                .interface_id(application.template)
                .expect("an application retains its declaration"),
            arguments,
        )
    }

    pub(super) fn lower_type(
        &mut self,
        source: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        match self.source.types[source].clone() {
            export::Type::Unit => self.intern_type(concrete::TypeKind::Unit, true),
            export::Type::Integer(kind) => {
                self.intern_type(concrete::TypeKind::Integer(kind), true)
            }
            export::Type::Boolean => self.intern_type(concrete::TypeKind::Boolean, true),
            export::Type::String => self.intern_type(concrete::TypeKind::String, false),
            export::Type::ImportedStruct(structure) => {
                self.lower_imported_struct(&structure, substitution)
            }
            export::Type::ImportedEnum(enumeration) => {
                self.lower_imported_enum(&enumeration, substitution)
            }
            export::Type::ImportedClass(class) => self.lower_imported_class(&class, substitution),
            export::Type::ImportedInterface(interface) => {
                self.lower_imported_interface(&interface, substitution)
            }
            export::Type::Struct(application) => {
                let value = self.source.struct_applications[application].clone();
                if matches!(
                    self.core,
                    export::CoreProtocols::Defined(protocols)
                        if value.template == self.source.nominal_identities[protocols.ffi.fun_ptr].declaration_id()
                ) {
                    let [function] = value.arguments.as_slice() else {
                        panic!("validated deferred FunPtr has one argument")
                    };
                    let function = self.lower_type(*function, substitution);
                    let concrete::TypeKind::Function(function) = self.types[function].kind else {
                        panic!("deferred FunPtr resolves to a concrete function type")
                    };
                    return self.intern_type(concrete::TypeKind::FunPtr(function), true);
                }
                let id = self.lower_struct_application(application, substitution);
                self.struct_type[&id]
            }
            export::Type::Class(application) => {
                let id = self.lower_class_application(application, substitution);
                self.class_type[&id]
            }
            export::Type::Interface(application) => {
                let id = self.lower_interface_application(application, substitution);
                self.interface_type[&id]
            }
            export::Type::Any => self.intern_type(concrete::TypeKind::Any, false),
            export::Type::Tuple(elements) => {
                let elements: Vec<_> = elements
                    .iter()
                    .map(|element| self.lower_type(*element, substitution))
                    .collect();
                let gc_free = elements.iter().all(|element| self.types[*element].gc_free);
                self.intern_type(concrete::TypeKind::Tuple(elements), gc_free)
            }
            export::Type::Function(id) => {
                let id = self.lower_function_type(id, substitution);
                self.function_types[id].canonical_type
            }
            export::Type::Ptr(pointee) => {
                let pointee = self.lower_type(pointee, substitution);
                self.intern_type(concrete::TypeKind::Ptr(pointee), true)
            }
            export::Type::FunPtr(id) => {
                let id = self.lower_function_type(id, substitution);
                self.intern_type(concrete::TypeKind::FunPtr(id), true)
            }
            export::Type::Enum(application) => {
                let id = self.lower_enum_application(application, substitution);
                self.enum_type[&id]
            }
            export::Type::Param(index) => substitution
                .get(index.into_raw() as usize)
                .copied()
                .expect("every local-concrete type parameter has a substitution"),
        }
    }

    pub(super) fn intern_type(
        &mut self,
        kind: concrete::TypeKind,
        gc_free: bool,
    ) -> concrete::TypeId {
        if let concrete::TypeKind::Function(function) = &kind {
            assert!(!gc_free, "managed function values are GC references");
            return self.function_types[*function].canonical_type;
        }
        if let Some(&id) = self.type_by_kind.get(&kind) {
            assert_eq!(
                self.types[id].gc_free, gc_free,
                "one concrete type identity has one GC-free classification"
            );
            return id;
        }
        let id = self.types.alloc(concrete::Type {
            kind: kind.clone(),
            gc_free,
        });
        self.type_by_kind.insert(kind, id);
        id
    }

    pub(super) fn lower_function_type(
        &mut self,
        source: export::FunctionTypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionTypeId {
        let source = self.source.function_types[source].clone();
        let parameter_types = source
            .parameter_types
            .iter()
            .map(|ty| self.lower_type(*ty, substitution))
            .collect::<Vec<_>>();
        let return_type = self.lower_type(source.return_type, substitution);
        let key = (source.is_suspend, parameter_types.clone(), return_type);
        if let Some(&id) = self.function_type_by_signature.get(&key) {
            return id;
        }
        let expected_id = concrete::FunctionTypeId::from_raw(
            u32::try_from(self.function_types.len())
                .expect("the concrete function-type arena fits its typed id")
                .into(),
        );
        let canonical_kind = concrete::TypeKind::Function(expected_id);
        assert!(!self.type_by_kind.contains_key(&canonical_kind));
        let canonical_type = self.types.alloc(concrete::Type {
            kind: canonical_kind.clone(),
            gc_free: false,
        });
        self.type_by_kind.insert(canonical_kind, canonical_type);
        let id = self.function_types.alloc(concrete::FunctionType {
            canonical_type,
            is_suspend: source.is_suspend,
            parameter_types,
            return_type,
        });
        assert_eq!(id, expected_id);
        self.function_type_by_signature.insert(key, id);
        id
    }
}
