use super::*;

impl Lowerer {
    pub(crate) fn enum_definition(&self, template: hir::SourceNominalId) -> &hir::EnumDefinition {
        match self.nominal_owners.get(&template) {
            Some(crate::Owner::Enum(id)) => &self.enums[*id].definition,
            Some(_) => unreachable!("an enum application retains its enum declaration"),
            None => &self.loaded_enum_definitions[&template].definition,
        }
    }

    pub(crate) fn dependency_nominal_application(
        &self,
        ty: TypeId,
    ) -> Option<(&std::sync::Arc<hir::ImportedNominalDeclaration>, &[TypeId])> {
        if let Type::Enum(application) = self.types[ty] {
            let application = &self.enum_applications[application];
            let definition = self.loaded_enum_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        self.types[ty].imported_nominal_application()
    }
}
