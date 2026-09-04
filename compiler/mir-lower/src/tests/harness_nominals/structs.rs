use super::super::*;

impl Harness {
    pub(in crate::tests) fn strukt(
        &mut self,
        name: &str,
        fields: &[(&str, hir::TypeId)],
    ) -> hir::StructId {
        self.strukt_with(name, fields, &[])
    }

    pub(in crate::tests) fn strukt_with(
        &mut self,
        name: &str,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        self.declare_struct(name, Vec::new(), Vec::new(), fields, interfaces)
    }

    pub(in crate::tests) fn declare_struct(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        assert_eq!(type_params.len(), self_arguments.len());
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let interface_implementations = self.interface_implementation_shells(&interfaces);
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let owner = hir::StructId::from_raw((self.structs.len() as u32).into());
        let parameters = fields
            .iter()
            .map(|(name, ty)| {
                let id = hir::ConstructorParamId::from_raw(self.next_constructor_param);
                self.next_constructor_param += 1;
                hir::ConstructorParameter {
                    id,
                    name: name.to_string(),
                    ty: *ty,
                }
            })
            .collect();
        let constructor = self.struct_constructors.alloc(hir::StructConstructor {
            owner,
            access: hir::DeclarationAccess::public(),
            parameters,
            kind: hir::StructConstructorKind::Primary,
            span: SPAN,
            origin: definition_origin(),
        });
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            access: hir::NominalAccess::public(),
            self_application,
            type_params,
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Declared(
                fields
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
            ),
            constructors: vec![constructor],
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let actual = self.struct_application(strukt, self_arguments);
        assert_eq!(actual, self_application);
        strukt
    }

    pub(in crate::tests) fn declare_fixed_intrinsic_struct(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        canonical_type: hir::TypeId,
    ) -> hir::StructId {
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            access: hir::NominalAccess::public(),
            self_application,
            type_params: Vec::new(),
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Intrinsic(declaration),
            constructors: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let representation = kind.application(&[]);
        let actual = self.struct_applications.alloc(hir::StructApplication {
            template: strukt,
            arguments: Vec::new(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        self.struct_applications_by_key
            .insert((strukt, Vec::new()), actual);
        strukt
    }

    pub(in crate::tests) fn declare_intrinsic_class(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        canonical_type_plan: CanonicalTypePlan,
    ) -> hir::ClassId {
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let class = self.classes.alloc(hir::ClassDecl {
            modifier: hir::ClassModifier::Final,
            name: name.to_string(),
            access: hir::NominalAccess::public(),
            self_application,
            type_params,
            representation: hir::ClassRepresentation::Intrinsic(declaration),
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: SPAN,
        });
        let representation = kind.application(&self_arguments);
        let (canonical_type, allocate_canonical_type) = match canonical_type_plan {
            CanonicalTypePlan::Existing(canonical_type) => (canonical_type, false),
            CanonicalTypePlan::Allocate => (
                hir::TypeId::from_raw((self.types.len() as u32).into()),
                true,
            ),
        };
        let actual = self.class_applications.alloc(hir::ClassApplication {
            template: class,
            arguments: self_arguments.clone(),
            canonical_type,
            representation: hir::ClassApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        if allocate_canonical_type {
            let allocated = self.types.alloc(hir::Type::Class(actual));
            assert_eq!(allocated, canonical_type);
        }
        self.class_applications_by_key
            .insert((class, self_arguments), actual);
        class
    }

    pub(in crate::tests) fn interface_implementation_shells(
        &self,
        interfaces: &[hir::TypeId],
    ) -> Vec<hir::InterfaceImplementation> {
        interfaces
            .iter()
            .map(|&interface| {
                let hir::Type::Interface(application) = self.types[interface] else {
                    panic!("test harness interface lists are fully applied")
                };
                let template = self.interface_applications[application].template;
                hir::InterfaceImplementation {
                    interface: application,
                    methods: self.interfaces[template]
                        .methods
                        .iter()
                        .map(|&member| hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        })
                        .collect(),
                }
            })
            .collect()
    }
}
