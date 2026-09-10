use super::*;

impl Lowerer {
    /// Intern a type, so structurally equal types share a single
    /// `TypeId` (this makes instantiation dedup a plain id comparison).
    /// Deduplication happens before allocation so the arena itself remains a
    /// canonical set and every entry can receive one persistent identity.
    pub(crate) fn intern_type(&mut self, candidate: Type) -> TypeId {
        if let Type::Function(function) = &candidate {
            return self.function_types[*function].canonical_type;
        }
        if let Some((existing, _)) = self.types.iter().find(|(_, value)| *value == &candidate) {
            return existing;
        }
        self.types.alloc(candidate)
    }

    /// Intern one complete function signature and return its ordinary HIR
    /// type. The signature arena and the surrounding `Type` arena are both
    /// canonical, so equal source spellings share both ids.
    pub(crate) fn intern_function_type(
        &mut self,
        is_suspend: bool,
        parameter_types: Vec<TypeId>,
        return_type: TypeId,
    ) -> TypeId {
        let existing = self.function_types.iter().find_map(|(id, candidate)| {
            (candidate.is_suspend == is_suspend
                && candidate.parameter_types == parameter_types
                && candidate.return_type == return_type)
                .then_some(id)
        });
        if let Some(function) = existing {
            return self.function_types[function].canonical_type;
        }

        let expected_function = hir::FunctionTypeId::from_raw(
            u32::try_from(self.function_types.len())
                .expect("the function-type arena fits its typed id")
                .into(),
        );
        let canonical_type = self.types.alloc(Type::Function(expected_function));
        let function = self.function_types.alloc(hir::FunctionType {
            canonical_type,
            is_suspend,
            parameter_types,
            return_type,
        });
        assert_eq!(function, expected_function);
        canonical_type
    }

    pub(super) fn instantiate_function_type(
        &mut self,
        id: hir::FunctionTypeId,
        mut substitute: impl FnMut(&mut Self, TypeId) -> Option<TypeId>,
    ) -> Option<TypeId> {
        let function = self.function_types[id].clone();
        let mut parameter_types = Vec::with_capacity(function.parameter_types.len());
        for parameter in function.parameter_types {
            parameter_types.push(substitute(self, parameter)?);
        }
        let return_type = substitute(self, function.return_type)?;
        Some(self.intern_function_type(function.is_suspend, parameter_types, return_type))
    }

    pub(crate) fn struct_application_id(
        &mut self,
        template: StructId,
        arguments: Vec<TypeId>,
    ) -> hir::StructApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.struct_application_by_key.get(&key) {
            return application;
        }
        let intrinsic = match self.structs[template].representation {
            hir::StructRepresentation::Declared(_) => None,
            hir::StructRepresentation::Intrinsic(intrinsic) => Some(intrinsic),
        };
        let representation = intrinsic.map_or(
            hir::StructApplicationRepresentation::Declared,
            |intrinsic| {
                hir::StructApplicationRepresentation::Intrinsic(
                    intrinsic.kind.application(&arguments),
                )
            },
        );
        let deferred_fun_ptr = matches!(
            &representation,
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::FunPtr { function }
            ) if matches!(self.types[*function], Type::Param(_))
        );
        let canonical_type = match &representation {
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Integer(kind),
            ) => self.integer_type(*kind),
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Boolean,
            ) => self.boolean,
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Ptr { pointee },
            ) => self.intern_type(Type::Ptr(*pointee)),
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::FunPtr { function },
            ) => match self.types[*function] {
                Type::Function(signature) => self.intern_type(Type::FunPtr(signature)),
                Type::Param(_) => hir::TypeId::from_raw((self.types.len() as u32).into()),
                _ => unreachable!("a concrete FunPtr argument is a function type"),
            },
            hir::StructApplicationRepresentation::Declared => {
                hir::TypeId::from_raw((self.types.len() as u32).into())
            }
            hir::StructApplicationRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic contract fixes its declaration target")
            }
        };
        let application = self.struct_applications.alloc(hir::StructApplication {
            template,
            arguments,
            canonical_type,
            representation,
        });
        if deferred_fun_ptr
            || matches!(
                self.struct_applications[application].representation,
                hir::StructApplicationRepresentation::Declared
            )
        {
            let allocated_type = self.types.alloc(Type::Struct(application));
            assert_eq!(allocated_type, canonical_type);
        }
        self.struct_application_by_key.insert(key, application);
        application
    }

    /// Intern a complete generic struct application (`PinnedPtr<String>`,
    /// M12). The type contains only the application identity; declaration and
    /// arguments live together in the application arena.
    pub(crate) fn struct_application(&mut self, template: StructId, args: Vec<TypeId>) -> TypeId {
        let application = self.struct_application_id(template, args);
        self.struct_applications[application].canonical_type
    }

    pub(crate) fn enum_application_id(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<TypeId>,
    ) -> hir::EnumApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.enum_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.enum_applications.alloc(hir::EnumApplication {
            template,
            arguments,
            canonical_type,
        });
        let allocated_type = self.types.alloc(Type::Enum(application));
        assert_eq!(allocated_type, canonical_type);
        self.enum_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn enum_application(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.enum_application_id(template, arguments);
        self.enum_applications[application].canonical_type
    }

    pub(crate) fn class_application_id(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<TypeId>,
    ) -> hir::ClassApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.class_application_by_key.get(&key) {
            return application;
        }
        let intrinsic = match self.classes[template].representation {
            hir::ClassRepresentation::Declared => None,
            hir::ClassRepresentation::Intrinsic(intrinsic) => Some(intrinsic),
        };
        let representation =
            intrinsic.map_or(hir::ClassApplicationRepresentation::Declared, |intrinsic| {
                hir::ClassApplicationRepresentation::Intrinsic(
                    intrinsic.kind.application(&arguments),
                )
            });
        let canonical_type = match &representation {
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::String,
            ) => self.string,
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Array { .. }
                | hir::IntrinsicTypeRepresentation::MutableArray { .. },
            )
            | hir::ClassApplicationRepresentation::Declared => {
                hir::TypeId::from_raw((self.types.len() as u32).into())
            }
            hir::ClassApplicationRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic contract fixes its declaration target")
            }
        };
        let application = self.class_applications.alloc(hir::ClassApplication {
            template,
            arguments,
            canonical_type,
            representation,
        });
        let allocate_type = !matches!(
            self.class_applications[application].representation,
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::String
            )
        );
        if allocate_type {
            let allocated_type = self.types.alloc(Type::Class(application));
            assert_eq!(allocated_type, canonical_type);
        }
        self.class_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn class_application(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.class_application_id(template, arguments);
        self.class_applications[application].canonical_type
    }

    pub(crate) fn class_constructor_application(
        &mut self,
        constructor: hir::ClassConstructorId,
        owner: hir::ClassApplicationId,
    ) -> hir::ClassConstructorApplicationId {
        let key = (constructor, owner);
        if let Some(&application) = self.class_constructor_application_by_key.get(&key) {
            return application;
        }
        debug_assert_eq!(
            self.class_constructors[constructor].owner,
            self.class_applications[owner].template
        );
        let application = self
            .class_constructor_applications
            .alloc(hir::ClassConstructorApplication { constructor, owner });
        self.class_constructor_application_by_key
            .insert(key, application);
        application
    }

    pub(crate) fn struct_constructor_application(
        &mut self,
        constructor: hir::StructConstructorId,
        owner: hir::StructApplicationId,
    ) -> hir::StructConstructorApplicationId {
        let key = (constructor, owner);
        if let Some(&application) = self.struct_constructor_application_by_key.get(&key) {
            return application;
        }
        debug_assert_eq!(
            self.struct_constructors[constructor].owner,
            self.struct_applications[owner].template
        );
        let application = self
            .struct_constructor_applications
            .alloc(hir::StructConstructorApplication { constructor, owner });
        self.struct_constructor_application_by_key
            .insert(key, application);
        application
    }

    pub(crate) fn interface_application_id(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<TypeId>,
    ) -> hir::InterfaceApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.interface_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template,
                arguments,
                canonical_type,
            });
        let allocated_type = self.types.alloc(Type::Interface(application));
        assert_eq!(allocated_type, canonical_type);
        self.interface_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn intern_interface_application(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.interface_application_id(template, arguments);
        self.interface_applications[application].canonical_type
    }
}
