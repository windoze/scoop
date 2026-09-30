use super::*;

impl Module {
    pub fn dependency_nominal_application(
        &self,
        ty: TypeId,
    ) -> Option<(&std::sync::Arc<ImportedNominalDeclaration>, &[TypeId])> {
        if let Type::Struct(application) = self.types[ty] {
            let application = &self.struct_applications[application];
            let definition = self.loaded_struct_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        if let Type::Enum(application) = self.types[ty] {
            let application = &self.enum_applications[application];
            let definition = self.loaded_enum_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        self.types[ty].imported_nominal_application()
    }
}
