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
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        for (template_index, template) in templates.iter().enumerate() {
            let path = path.clone().index(template_index as u64).field(11).field(3);
            self.meter.charge_nodes(1, &path)?;
            self.meter
                .check_table_entries(template.references().types().len() as u64, &path)?;
            for (index, reference) in template.references().types().iter().enumerate() {
                let path = path.clone().index(index as u64);
                let mut validate = || -> Result<(), Error> {
                    let expected = self.source_type_access_domain(
                        reference.target(),
                        current_protocols,
                        &path,
                    )?;
                    let actual = reference.witness().target_domain();
                    let cost = scoop_wire::encoded_length(&expected)
                        .and_then(|left| {
                            scoop_wire::encoded_length(actual)
                                .map(|right| left.saturating_add(right))
                        })
                        .map_err(|error| Error::Encoding(error.to_string()))?;
                    self.meter.charge_work(cost, &path)?;
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
