//! Expression origins remain tied to the same current/dependency foundations.

use super::*;
use scoop_wire::WirePath;

mod errors;
mod generated;
pub use errors::CrossConeHirTypeSiteError;

impl HirInterfaceValidationInput<'_> {
    pub(crate) fn type_sites(
        self,
        dependencies: &[ValidatedNominalProviderView<'_>],
    ) -> Result<(), CrossConeHirTypeSiteError> {
        let path = WirePath::root().field(10);
        let mut providers = Vec::new();

        scoop_wire::allocation::try_reserve(&mut providers, dependencies.len(), &path)?;
        providers.extend(dependencies.iter().map(|view| view.identity));
        self.interface
            .external_references()
            .validate_type_site_relations(self.current, self.identities, &providers)
            .map_err(|error| CrossConeHirTypeSiteError::Relations(Box::new(error)))?;
        for reference in self.interface.external_references().records() {
            for site in reference.type_sites().records() {
                match site {
                    scoop_hir::HirDependencyTypeSiteV1::Expression(site) => self
                        .executable_origin(site.position(), site.origin(), dependencies)
                        .map_err(|error| CrossConeHirTypeSiteError::Origin(Box::new(error)))?,
                    _ => self
                        .foundation
                        .validate_declaration_type_position(self.current, site.position())
                        .map_err(|error| CrossConeHirTypeSiteError::Declaration(Box::new(error)))?,
                }
                self.generated_type_site(reference.origin(), site, dependencies)?;
            }
        }
        Ok(())
    }
}
