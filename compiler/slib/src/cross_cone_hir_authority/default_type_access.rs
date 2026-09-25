//! Type access is derived from declarations, never from a reference's claim.

use scoop_hir::CoreBootstrapInterfaceSectionV1;
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultTypeAccessError;
type Error = CrossConeHirDefaultTypeAccessError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_type_access(
        &mut self,
        current_protocols: &CoreBootstrapInterfaceSectionV1,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();

        for (template_index, template) in templates.iter().enumerate() {
            let path = path.clone().index(template_index as u64).field(11).field(3);

            for (index, reference) in template.references().types().iter().enumerate() {
                let path = path.clone().index(index as u64);
                let mut validate = || -> Result<(), Error> {
                    let expected = self.source_type_access_domain(
                        reference.target(),
                        current_protocols,
                        &path,
                    )?;
                    let actual = reference.witness().target_domain();

                    if actual != &expected {
                        return Err(Error::WitnessDomain);
                    }
                    Ok(())
                };
                validate().map_err(|source| Error::Reference {
                    template: template.key(),
                    index,
                    source: Box::new(source),
                })?;
            }
        }
        Ok(())
    }
}
