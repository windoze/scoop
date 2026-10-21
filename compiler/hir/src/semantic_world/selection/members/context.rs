use super::*;

impl ImportedDependencySelectionPlan {
    pub fn nominal_context_declarations(
        &self,
        owner: SourceNominalId,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        self.catalog
            .indexes
            .nominals
            .get(&owner)
            .into_iter()
            .flat_map(|members| &members.contextual)
            .map(|&declaration| self.callable_declaration(declaration))
            .collect()
    }
}
