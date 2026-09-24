//! Expression origins remain tied to the same current/dependency foundations.

use super::*;
use scoop_wire::WirePath;

mod errors;
pub use errors::CrossConeHirTypeSiteError;

impl HirInterfaceValidationInput<'_> {
    pub(crate) fn type_sites(
        self,
        dependencies: &[ValidatedNominalProviderView<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<(), CrossConeHirTypeSiteError> {
        let path = WirePath::root().field(10);
        let mut providers = Vec::new();
        meter.charge_owned_bytes(
            (dependencies.len() * std::mem::size_of::<ConeIdentity>()) as u64,
            &path,
        )?;
        meter.try_reserve_collection_slots(&mut providers, dependencies.len(), &path)?;
        providers.extend(dependencies.iter().map(|view| view.identity));
        self.interface
            .external_references()
            .validate_type_site_relations(self.current, self.identities, &providers, meter)
            .map_err(|error| CrossConeHirTypeSiteError::Relations(Box::new(error)))?;
        for (index, reference) in self
            .interface
            .external_references()
            .records()
            .iter()
            .enumerate()
        {
            meter.charge_work(1, &path)?;
            for (site_index, site) in reference.type_sites().records().iter().enumerate() {
                let path = path
                    .clone()
                    .index(index as u64)
                    .field(6)
                    .index(site_index as u64);
                self.executable_origin(site.position(), site.origin(), dependencies, meter, &path)
                    .map_err(|error| CrossConeHirTypeSiteError::Origin(Box::new(error)))?;
            }
        }
        Ok(())
    }
}
