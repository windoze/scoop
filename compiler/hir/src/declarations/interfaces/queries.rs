use super::*;

impl Module {
    pub fn dependency_interface_definition(
        &self,
        ty: TypeId,
    ) -> Option<&LoadedInterfaceDefinition> {
        let Type::Interface(application) = self.types[ty] else {
            return None;
        };
        self.loaded_interface_definitions
            .get(&self.interface_applications[application].template)
    }

    pub fn interface_definition(&self, template: SourceNominalId) -> &InterfaceDefinition {
        match self.nominal_identities.interface_id(template) {
            Some(id) => &self.interfaces[id].definition,
            None => &self.loaded_interface_definitions[&template].definition,
        }
    }

    pub fn interface_name(&self, template: SourceNominalId) -> &str {
        match self.nominal_identities.interface_id(template) {
            Some(id) => &self.interfaces[id].name,
            None => self.loaded_interface_definitions[&template]
                .declaration
                .name(),
        }
    }
}
