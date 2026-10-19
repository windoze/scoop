use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_companion_type(
        &mut self,
        host: concrete::TypeId,
    ) -> concrete::TypeId {
        let owner = self
            .type_nominal(host)
            .expect("a companion has a nominal host");
        let arguments = self.type_arguments(host);
        let local = self.source.nominal_identities.declaration(owner);
        if let Some((_, relation)) = self
            .source
            .companion_relations
            .iter()
            .find(|(_, relation)| Some(relation.host) == local)
        {
            let object = &self.source.objects[relation.object];
            let source = self.source.class_applications
                [self.source.classes[object.backing_class].self_application]
                .canonical_type;
            return self.lower_type(source, &arguments);
        }
        let source = self.source.loaded_class_definitions.values().find(|definition| {
            matches!(definition.declaration.interface.source_shape(), export::NominalSourceShapeV1::Object(shape)
                if shape.object_kind() == export::ObjectSourceKindV1::Companion)
                && definition.declaration.identity.declaration().owners().owners().last()
                    == Some(&match owner {
                        export::SourceNominalId::Concrete(id) => scoop_identity::DefinitionOwnerAtom::Type(id),
                        export::SourceNominalId::GenericTemplate(id) => scoop_identity::DefinitionOwnerAtom::GenericType(id),
                    })
        }).expect("checked companion conditions retain the companion declaration");
        let source =
            self.source.class_applications[source.definition.self_application].canonical_type;
        self.lower_type(source, &arguments)
    }

    pub(in crate::concretize) fn projected_singleton_value(
        &self,
        ty: concrete::TypeId,
    ) -> concrete::SingletonValueTarget {
        let concrete::TypeKind::Class(class) = self.types[ty].kind else {
            unreachable!("a companion is a reference singleton")
        };
        if self.object_type_map.contains_key(&class) {
            return concrete::SingletonValueTarget::Local(self.singleton_value_for_type(ty));
        }
        let definition =
            &self.source.loaded_class_definitions[&self.classes[class].origin.declaration_id()];
        let export::NominalSourceShapeV1::Object(shape) =
            definition.declaration.interface.source_shape()
        else {
            unreachable!("a companion retains its object declaration")
        };
        concrete::SingletonValueTarget::Dependency(shape.value())
    }
}
