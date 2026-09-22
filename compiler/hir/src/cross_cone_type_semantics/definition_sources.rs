//! Exact inline-origin closure, independent of public lookup and selection.
use crate::{
    CanonicalExportDefinitionSourcesV1, CanonicalNominalInheritanceInterfacesV1,
    CanonicalNominalRepresentationSupportV1, CanonicalProtectedCallableSourceInterfacesV1,
    CanonicalProtectedDeclarationInterfacesV1, CanonicalProtectedDefaultTemplatesV1,
};
use scoop_wire::{BudgetMeter, WirePath};

mod collection;
mod declarations;
mod defaults;
mod errors;
mod uses;
mod validation;
pub use errors::*;
pub use uses::*;

trait SourceVisitor<E> {
    fn observe(
        &mut self,
        source: &crate::ExportDefinitionSourceV1,
        source_use: TypeDefinitionSourceUseV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<E>>;
}

/// The actual origin-bearing fields of one candidate type section. This view
/// is transport data and grants neither declaration access nor publication.
#[derive(Clone, Copy, Debug)]
pub struct TypeDefinitionSourceInputsV1<'a> {
    pub representations: &'a CanonicalNominalRepresentationSupportV1,
    pub inheritance: &'a CanonicalNominalInheritanceInterfacesV1,
    pub protected_declarations: &'a CanonicalProtectedDeclarationInterfacesV1,
    pub source_interfaces: &'a CanonicalProtectedCallableSourceInterfacesV1,
    pub defaults: &'a CanonicalProtectedDefaultTemplatesV1,
}
impl TypeDefinitionSourceInputsV1<'_> {
    /// Replays every typed occurrence and requires exactly its deduplicated
    /// origin set. Provider/source membership comes from the independent authority.
    pub fn validate_definition_sources<A: TypeDefinitionSourceSemanticAuthority<E>, E>(
        self,
        declared: &CanonicalExportDefinitionSourcesV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<E>> {
        let mut validator =
            validation::Validator::new(declared, authority, meter, &path.clone().field(7))?;
        declarations::visit(self, &mut validator, meter, path)?;
        defaults::visit(self, &mut validator, meter, path)?;
        validator.finish(meter, path)
    }
}

#[cfg(test)]
mod tests;
