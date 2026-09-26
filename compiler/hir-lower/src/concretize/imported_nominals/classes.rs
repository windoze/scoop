use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_imported_class(
        &mut self,
        source: &export::ImportedClassType,
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let identity = declaration.identity.id();
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
        let allocated = self.classes.alloc(concrete::ClassDef {
            origin: export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Concrete(
                declaration.identity.clone(),
            )),
            canonical_type: ty,
            modifier,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: Vec::new(),
            representation: concrete::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            // Dependency method bodies remain in the provider.
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
                ty: self.lower_type(field.ty, &[]),
            })
            .collect();
        let base_class = source.base_class.map(|base| {
            let base = self.lower_type(base, &[]);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("a resolved class base retains its class type")
            };
            base
        });
        let interfaces = source
            .interfaces
            .iter()
            .map(|ty| self.lower_type(*ty, &[]))
            .collect();
        self.classes[id].representation =
            concrete::ClassRepresentation::Declared { fields, base_class };
        self.classes[id].interfaces = interfaces;
        ty
    }
}
