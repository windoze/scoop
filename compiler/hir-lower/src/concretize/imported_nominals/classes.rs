use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_imported_class(
        &mut self,
        source: &export::ImportedClassType,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let identity = (declaration.owner(), arguments.clone());
        if let Some(id) = self.imported_classes.get(&identity) {
            return self.class_type[id];
        }
        let modifier = match declaration.interface.declaration_details().modality() {
            export::NominalInheritanceModalityV1::Final => export::ClassModifier::Final,
            export::NominalInheritanceModalityV1::Open => export::ClassModifier::Open,
            export::NominalInheritanceModalityV1::Abstract => export::ClassModifier::Abstract,
            export::NominalInheritanceModalityV1::Interface => {
                unreachable!("a source class has class modality")
            }
        };
        let id = concrete::ClassId::from_raw(
            u32::try_from(self.classes.len())
                .expect("concrete class ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Class(id), false);
        let representation = match declaration.interface.source_shape() {
            export::NominalSourceShapeV1::Intrinsic(source) => {
                let [element] = arguments.as_slice() else {
                    unreachable!("intrinsic array arity was checked in HIR")
                };
                let application = match source.family() {
                    export::IntrinsicTypeKind::Array => {
                        concrete::IntrinsicTypeRepresentation::Array { element: *element }
                    }
                    export::IntrinsicTypeKind::MutableArray => {
                        concrete::IntrinsicTypeRepresentation::MutableArray { element: *element }
                    }
                    _ => unreachable!(
                        "imported intrinsic classes retain their resolved array family"
                    ),
                };
                concrete::ClassRepresentation::Intrinsic {
                    declaration: source.family(),
                    application,
                }
            }
            _ => concrete::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
        };
        let allocated = self.classes.alloc(concrete::ClassDef {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            canonical_type: ty,
            modifier,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: arguments,
            representation,
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            // Dispatch is completed after the exact receiver has been registered.
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_classes.insert(identity, id);
        self.class_type.insert(id, ty);

        let fields = source
            .fields
            .iter()
            .map(|field| concrete::Field {
                identity: field.identity,
                name: field.name.clone(),
                ty: self.lower_type(field.ty, substitution),
            })
            .collect();
        let base_class = source.base_class.map(|base| {
            let base = self.lower_type(base, substitution);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("a resolved class base retains its class type")
            };
            base
        });
        let interfaces = source
            .interfaces
            .iter()
            .map(|ty| self.lower_type(*ty, substitution))
            .collect();
        if matches!(
            self.classes[id].representation,
            concrete::ClassRepresentation::Declared { .. }
        ) {
            self.classes[id].representation =
                concrete::ClassRepresentation::Declared { fields, base_class };
        }
        self.classes[id].direct_interfaces = interfaces;
        self.classes[id].methods = source
            .virtual_methods
            .iter()
            .map(|method| match method.callable {
                export::ImportedDispatchCallable::External(callable) => {
                    concrete::ClassMethod::Imported {
                        family: self.lower_virtual_method(method.family),
                        callable: self.imported_dependency_callable_map[&callable],
                    }
                }
                export::ImportedDispatchCallable::Template(application) => {
                    concrete::ClassMethod::Local(self.lower_imported_callable_application(
                        &self.source.imported_generic_applications[application],
                        substitution,
                    ))
                }
            })
            .collect();
        self.classes[id].interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, substitution);
        self.classes[id].interfaces = self.classes[id]
            .interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        ty
    }
}
