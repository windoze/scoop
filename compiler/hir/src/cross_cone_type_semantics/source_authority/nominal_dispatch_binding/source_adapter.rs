//! Section declaration queries preserve the complete artifact source chain.
use super::*;
use scoop_identity::*;
use std::borrow::Cow;

mod callables;
mod errors;
mod graph;
mod interfaces;
mod nominals;
mod properties;
mod schemas;
mod shapes;

impl ProtectedDeclarationSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn required_protected_declarations(
        &self,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, Error> {
        Ok(&self.required)
    }
}
impl TypeSectionDeclarationSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn validate_protected_sources<'c>(
        &mut self,
        table: &'c CanonicalProtectedDeclarationInterfacesV1,
        protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
        representations: &'c CanonicalNominalRepresentationSupportV1,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
    ) -> Result<CheckedProtectedDeclarationSourcesV1<'c>, ProtectedDeclarationSemanticError<Error>>
    {
        // This Copy duplicates only immutable source borrows and a typed id.
        let mut parameters = *self.parameters;
        parameters
            .validate_protected_declarations(table, protocols, representations)
            .map_err(|error| match Error::from(error) {
                Error::Resource(error) => ProtectedDeclarationSemanticError::Resource(error),
                error => ProtectedDeclarationSemanticError::Foundation(error),
            })?;
        table.validate_sources(graph, representations, self)
    }
}
