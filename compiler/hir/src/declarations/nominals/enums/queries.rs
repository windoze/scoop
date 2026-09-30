use super::*;

impl LoadedEnumDefinition {
    pub fn variant_identity(&self, index: usize) -> scoop_identity::PersistentEnumVariantId {
        let NominalSourceShapeV1::Enum(shape) = self.declaration.interface.source_shape() else {
            unreachable!("an enum definition retains its enum declaration")
        };
        shape.variants()[index].variant()
    }

    pub fn variant_index(&self, variant: scoop_identity::PersistentEnumVariantId) -> Option<usize> {
        let NominalSourceShapeV1::Enum(shape) = self.declaration.interface.source_shape() else {
            unreachable!("an enum definition retains its enum declaration")
        };
        shape
            .variants()
            .iter()
            .position(|candidate| candidate.variant() == variant)
    }

    pub fn field_identity(
        &self,
        variant: usize,
        field: usize,
    ) -> scoop_identity::PersistentEnumVariantFieldId {
        let NominalSourceShapeV1::Enum(shape) = self.declaration.interface.source_shape() else {
            unreachable!("an enum definition retains its enum declaration")
        };
        shape.variants()[variant].fields()[field].field()
    }

    pub fn field_index(
        &self,
        variant: usize,
        field: scoop_identity::PersistentEnumVariantFieldId,
    ) -> Option<usize> {
        let NominalSourceShapeV1::Enum(shape) = self.declaration.interface.source_shape() else {
            unreachable!("an enum definition retains its enum declaration")
        };
        shape.variants()[variant]
            .fields()
            .iter()
            .position(|candidate| candidate.field() == field)
    }
}

impl Module {
    pub fn enum_definition(&self, template: SourceNominalId) -> &EnumDefinition {
        match self.nominal_identities.enum_id(template) {
            Some(id) => &self.enums[id].definition,
            None => &self.loaded_enum_definitions[&template].definition,
        }
    }

    pub fn enum_name(&self, template: SourceNominalId) -> &str {
        match self.nominal_identities.enum_id(template) {
            Some(id) => &self.enums[id].name,
            None => self.loaded_enum_definitions[&template].declaration.name(),
        }
    }

    pub fn enum_variant_index(&self, application: EnumVariantApplication) -> usize {
        let Type::Enum(owner) = self.types[application.owner] else {
            unreachable!("an enum variant retains its complete enum owner")
        };
        let owner = self.enum_applications[owner].template;
        match self.loaded_enum_definitions.get(&owner) {
            Some(definition) => definition
                .variant_index(application.variant)
                .expect("a variant belongs to its declaring enum"),
            None => self
                .enum_member_identities
                .variant_declaration(application.variant)
                .expect("a current variant retains its declaration identity")
                .local_index() as usize,
        }
    }

    pub fn enum_field_index(&self, application: EnumVariantFieldApplication) -> usize {
        let Type::Enum(owner) = self.types[application.variant.owner] else {
            unreachable!("an enum field retains its complete enum owner")
        };
        let owner = self.enum_applications[owner].template;
        match self.loaded_enum_definitions.get(&owner) {
            Some(definition) => definition
                .field_index(
                    self.enum_variant_index(application.variant),
                    application.field,
                )
                .expect("a field belongs to its declaring variant"),
            None => self
                .enum_member_identities
                .field_declaration(application.field)
                .expect("a current field retains its declaration identity")
                .local_index() as usize,
        }
    }

    pub fn dependency_nominal_application(
        &self,
        ty: TypeId,
    ) -> Option<(&std::sync::Arc<ImportedNominalDeclaration>, &[TypeId])> {
        if let Type::Enum(application) = self.types[ty] {
            let application = &self.enum_applications[application];
            let definition = self.loaded_enum_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        self.types[ty].imported_nominal_application()
    }
}
