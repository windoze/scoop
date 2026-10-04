use super::*;

impl ImportedDependencySelectionPlan {
    pub fn nominal_context_declarations(
        &self,
        owner: SourceNominalId,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        self.catalog
            .callables
            .values()
            .filter(|entry| {
                entry.interface.owner() == PublicDeclarationOwnerV1::Nominal(owner)
                    && !entry.interface.context_parameters().is_empty()
                    && entry.interface.type_parameters().len_u32() == 0
            })
            .map(|entry| self.callable_declaration(entry.interface.declaration()))
            .collect()
    }
}
