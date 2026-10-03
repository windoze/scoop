use super::super::*;

impl Harness {
    /// `Option<inner>` (core's enum applied to one argument).
    pub(in crate::tests) fn option(&mut self, inner: hir::TypeId) -> hir::TypeId {
        let application = self.enum_application(self.option_enum, vec![inner]);
        self.enum_applications[application].canonical_type
    }

    pub(in crate::tests) fn any(&mut self) -> hir::TypeId {
        self.types.alloc(hir::Type::Any)
    }

    pub(in crate::tests) fn class_ty(&mut self, id: hir::ClassId) -> hir::TypeId {
        let application = self.class_application(id, Vec::new());
        self.class_applications[application].canonical_type
    }

    pub(in crate::tests) fn interface_ty(&mut self, id: hir::InterfaceId) -> hir::TypeId {
        self.interface_app(id, Vec::new())
    }

    pub(in crate::tests) fn interface_app(
        &mut self,
        id: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::TypeId {
        assert_eq!(self.interfaces[id].type_params.len(), arguments.len());
        let application = self.interface_application(id, arguments);
        self.interface_applications[application].canonical_type
    }

    pub(in crate::tests) fn struct_ty(&mut self, id: hir::StructId) -> hir::TypeId {
        self.struct_app(id, Vec::new())
    }

    pub(in crate::tests) fn enum_ty(&mut self, id: hir::EnumId) -> hir::TypeId {
        assert!(self.enums[id].type_params.is_empty());
        let application = self.enum_application(id, Vec::new());
        self.enum_applications[application].canonical_type
    }

    pub(in crate::tests) fn struct_application_of(
        &self,
        ty: hir::TypeId,
    ) -> hir::StructApplicationId {
        let hir::Type::Struct(application) = self.types[ty] else {
            panic!("expected a struct application type")
        };
        application
    }

    pub(in crate::tests) fn struct_field_ref(
        &self,
        application: hir::StructApplicationId,
        index: u32,
    ) -> hir::FieldRef {
        let owner = &self.struct_applications[application];
        let reference =
            hir::StructFieldRef::checked(&self.structs, self.struct_id(owner.template), index)
                .expect("test field belongs to its declaring struct");
        let nominals = super::test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        );
        let field = hir::HirFieldIdentityBuilder::default()
            .struct_field(&self.structs, &nominals, reference)
            .expect("test field has its declaration identity");
        hir::FieldRef::StructField {
            owner: owner.canonical_type,
            field,
        }
    }

    pub(in crate::tests) fn class_field_ref(
        &self,
        application: hir::ClassApplicationId,
        field: hir::ClassFieldId,
    ) -> hir::FieldRef {
        let nominals = super::test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        );
        let properties =
            super::test_property_identities(&self.properties, &Arena::new(), &nominals);
        let declaration = &self.class_fields[field];
        let field = hir::HirFieldIdentityBuilder::default()
            .class_field(
                field,
                declaration,
                &Arena::new(),
                &self.properties,
                &Arena::new(),
                &nominals,
                &properties[declaration.property],
            )
            .expect("test field has its storage identity");
        hir::FieldRef::ClassField {
            owner: self.class_applications[application].canonical_type,
            field,
        }
    }

    pub(in crate::tests) fn enum_application_of(&self, ty: hir::TypeId) -> hir::EnumApplicationId {
        let hir::Type::Enum(application) = self.types[ty] else {
            panic!("expected an enum application type")
        };
        application
    }

    pub(in crate::tests) fn enum_variant_ref(
        &self,
        application: hir::EnumApplicationId,
        index: u32,
    ) -> hir::EnumVariantApplication {
        let owner = &self.enum_applications[application];
        let declaration =
            hir::EnumVariantRef::checked(&self.enums, self.enum_id(owner.template), index)
                .expect("test variant belongs to its declaring enum");
        let nominals = super::test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        );
        let members = hir::HirEnumMemberIdentities::from_declarations(&self.enums, &nominals)
            .expect("test variants have their original declaration identities");
        hir::EnumVariantApplication {
            owner: owner.canonical_type,
            variant: members[declaration].id(),
        }
    }

    pub(in crate::tests) fn class_application_of(
        &self,
        ty: hir::TypeId,
    ) -> hir::ClassApplicationId {
        let hir::Type::Class(application) = self.types[ty] else {
            panic!("expected a class application type")
        };
        application
    }

    pub(in crate::tests) fn struct_application(
        &mut self,
        template: hir::StructId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::StructApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.struct_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let declaration = self.nominal_identities()[template].declaration_id();
        let application = self.struct_applications.alloc(hir::StructApplication {
            template: declaration,
            arguments: key.1.clone(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Declared,
        });
        let actual_type = self.types.alloc(hir::Type::Struct(application));
        assert_eq!(actual_type, canonical_type);
        self.struct_applications_by_key.insert(key, application);
        for &constructor in &self.structs[template].constructors {
            self.struct_constructor_applications
                .alloc(hir::StructConstructorApplication {
                    constructor: constructor.into(),
                    owner: application,
                });
        }
        application
    }

    pub(in crate::tests) fn enum_application(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::EnumApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.enum_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let declaration = self.nominal_identities()[template].declaration_id();
        let application = self.enum_applications.alloc(hir::EnumApplication {
            template: declaration,
            arguments: key.1.clone(),
            canonical_type,
        });
        let actual_type = self.types.alloc(hir::Type::Enum(application));
        assert_eq!(actual_type, canonical_type);
        self.enum_applications_by_key.insert(key, application);
        application
    }

    pub(in crate::tests) fn declare_enum(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        variants: Vec<hir::Variant>,
    ) -> hir::EnumId {
        assert_eq!(type_params.len(), self_arguments.len());
        let self_application =
            hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
        let enumeration = self.enums.alloc(hir::EnumDecl {
            owner: None,
            name: name.to_string(),
            access: hir::NominalAccess::public(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: SPAN,

            definition: hir::EnumDefinition {
                self_application,
                type_params,
                variants,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),

                gc_free_pointee_requirements: Vec::new(),
                no_gc: false,
            },
        });
        let actual = self.enum_application(enumeration, self_arguments);
        assert_eq!(actual, self_application);
        enumeration
    }

    pub(in crate::tests) fn class_application(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::ClassApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.class_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let representation = match self.classes[template].representation {
            hir::ClassRepresentation::Declared => hir::ClassApplicationRepresentation::Declared,
            hir::ClassRepresentation::Intrinsic(declaration) => {
                hir::ClassApplicationRepresentation::Intrinsic(declaration.application(&key.1))
            }
        };
        let declaration = self.nominal_identities()[template].declaration_id();
        let application = self.class_applications.alloc(hir::ClassApplication {
            template: declaration,
            arguments: key.1.clone(),
            canonical_type,
            representation,
        });
        let actual_type = self.types.alloc(hir::Type::Class(application));
        assert_eq!(actual_type, canonical_type);
        self.class_applications_by_key.insert(key, application);
        for &constructor in &self.classes[template].constructors {
            self.class_constructor_applications
                .alloc(hir::ClassConstructorApplication {
                    constructor: constructor.into(),
                    owner: application,
                });
        }
        application
    }

    pub(in crate::tests) fn interface_application(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::InterfaceApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.interface_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let declaration = self.nominal_identities()[template].declaration_id();
        let application = self
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template: declaration,
                arguments: key.1.clone(),
                canonical_type,
            });
        let actual_type = self.types.alloc(hir::Type::Interface(application));
        assert_eq!(actual_type, canonical_type);
        self.interface_applications_by_key.insert(key, application);
        application
    }
}
