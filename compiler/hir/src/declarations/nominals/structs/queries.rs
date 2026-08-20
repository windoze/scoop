use super::*;

impl LoadedStructDefinition {
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
    pub fn struct_definition(&self, template: SourceNominalId) -> &StructDefinition {
        match self.nominal_identities.struct_id(template) {
            Some(id) => &self.structs[id].definition,
            None => &self.loaded_struct_definitions[&template].definition,
        }
    }

    pub fn struct_name(&self, template: SourceNominalId) -> &str {
        match self.nominal_identities.struct_id(template) {
            Some(id) => &self.structs[id].name,
            None => self.loaded_struct_definitions[&template].declaration.name(),
        }
    }
}
