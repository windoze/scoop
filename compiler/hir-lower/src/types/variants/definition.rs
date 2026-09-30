use super::*;

impl Lowerer {
    pub(crate) fn enum_definition(&self, template: hir::SourceNominalId) -> &hir::EnumDefinition {
        match self.nominal_owners.get(&template) {
            Some(crate::Owner::Enum(id)) => &self.enums[*id].definition,
            Some(_) => unreachable!("an enum application retains its enum declaration"),
            None => &self.loaded_enum_definitions[&template].definition,
        }
    }
}
