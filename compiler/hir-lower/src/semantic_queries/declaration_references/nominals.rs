use super::*;

impl DeclarationReferences {
    pub(super) fn nominals(&mut self, lowerer: &mut Lowerer) {
        let owners = lowerer
            .nominal_owners
            .iter()
            .map(|(identity, owner)| (*identity, *owner))
            .collect::<Vec<_>>();
        for (identity, owner) in owners {
            let span = match owner {
                Owner::Struct(id) => lowerer.structs[id].span,
                Owner::Class(id) => lowerer.classes[id].span,
                Owner::Enum(id) => lowerer.enums[id].span,
                Owner::Interface(id) => lowerer.interfaces[id].span,
                Owner::Object(id) => lowerer.objects[id].span,
            };
            self.current(
                Target::Nominal(identity),
                Source::Nominal(owner.as_nominal_owner()),
                lowerer.query_member_location(owner, span),
            );
            match owner {
                Owner::Struct(id) => self.struct_fields(lowerer, id),
                Owner::Class(id) => self.class_fields(lowerer, id),
                Owner::Enum(id) => self.variants(lowerer, id),
                Owner::Interface(_) | Owner::Object(_) => {}
            }
        }
    }

    fn struct_fields(&mut self, lowerer: &mut Lowerer, structure: hir::StructId) {
        let declaration = &lowerer.structs[structure];
        let application = declaration.definition.self_application;
        let count = declaration.definition.semantic_fields().len();
        for index in 0..count {
            let index = index as u32;
            let source = hir::StructFieldRef::checked(&lowerer.structs, structure, index)
                .expect("actual source field");
            let hir::FieldRef::StructField { field, .. } =
                lowerer.struct_field_reference(application, index)
            else {
                unreachable!("struct fields keep their declaration kind")
            };
            self.current(
                Target::Field(field),
                Source::Field(source),
                lowerer.query_member_location(
                    Owner::Struct(structure),
                    lowerer.struct_field_spans[&source],
                ),
            );
        }
    }

    fn class_fields(&mut self, lowerer: &mut Lowerer, class: hir::ClassId) {
        for source in lowerer.classes[class].fields.clone() {
            let identity = lowerer.class_field_identity(source);
            self.current(
                Target::Field(identity),
                Source::ClassField(source),
                lowerer
                    .query_member_location(Owner::Class(class), lowerer.class_fields[source].span),
            );
        }
    }

    fn variants(&mut self, lowerer: &Lowerer, enumeration: hir::EnumId) {
        let identities = lowerer
            .enum_member_identities
            .as_ref()
            .expect("declarations have variant identities");
        for (index, variant) in lowerer.enums[enumeration].variants.iter().enumerate() {
            let source = hir::EnumVariantRef::checked(&lowerer.enums, enumeration, index as u32)
                .expect("actual source variant");
            let id = identities[source].id();
            let origin = lowerer.query_member_location(
                Owner::Enum(enumeration),
                lowerer.enum_variant_spans[&source],
            );
            self.current(Target::Variant(id), Source::Variant(source), origin.clone());
            self.current(
                Target::Callable(hir::AnnotationCallableV1::Declaration(
                    scoop_identity::CallableTemplateOrigin::VariantConstructor(id),
                )),
                Source::Callable(hir::SourceAnnotationCallable::VariantConstructor(source)),
                origin,
            );
            for field_index in 0..variant.fields.len() {
                let field =
                    hir::EnumVariantFieldRef::checked(&lowerer.enums, source, field_index as u32)
                        .expect("actual variant field");
                self.current(
                    Target::VariantField(identities[field].id()),
                    Source::VariantField(field),
                    lowerer.query_member_location(
                        Owner::Enum(enumeration),
                        lowerer.enum_variant_field_spans[&field],
                    ),
                );
            }
        }
    }
}
