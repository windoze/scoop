use super::*;

impl LoadedClassDefinition {
    pub fn field_identity(&self, index: usize) -> scoop_identity::PersistentFieldId {
        self.declaration.interface.source_shape().declared_fields()[index].field()
    }

    pub fn field_index(&self, field: scoop_identity::PersistentFieldId) -> Option<usize> {
        self.declaration
            .interface
            .source_shape()
            .declared_fields()
            .iter()
            .position(|candidate| candidate.field() == field)
    }
}

impl Module {
    pub fn class_definition(&self, template: SourceNominalId) -> &ClassDefinition {
        match self.nominal_identities.class_id(template) {
            Some(id) => &self.classes[id].definition,
            None => &self.loaded_class_definitions[&template].definition,
        }
    }

    pub fn class_name(&self, template: SourceNominalId) -> &str {
        match self.nominal_identities.class_id(template) {
            Some(id) => &self.classes[id].name,
            None => self.loaded_class_definitions[&template].declaration.name(),
        }
    }

    pub fn class_field_definition(&self, field: ClassFieldId) -> &Field {
        let source = &self.class_fields[field];
        &self.classes[source.owner].definition.fields[source.definition_index]
    }
}
