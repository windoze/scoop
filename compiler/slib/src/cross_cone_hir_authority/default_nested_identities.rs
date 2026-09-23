//! Binds actual nested descriptors to their provider's shared artifact keys.

use scoop_hir::DefaultTargetIdentityQueriesV1;
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultNestedIdentityError;
type Error = CrossConeHirDefaultNestedIdentityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_nested_identities(&mut self) -> Result<(), Error> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        for (index, template) in templates.iter().enumerate() {
            let path = path.clone().index(index as u64);
            let mut validate = || -> Result<(), Error> {
                let nested = template.index_nested_callables(self.meter, &path)?;
                for occurrence in nested.occurrences() {
                    let origin = occurrence.definition_origin();
                    let provider = origin.origin().source().cone();
                    self.meter
                        .charge_work(self.dependencies.len() as u64 + 1, &path)?;
                    let foundation = if provider == self.current {
                        self.current_foundation
                    } else {
                        self.dependencies
                            .iter()
                            .find(|entry| entry.identity == provider)
                            .map(|entry| entry.foundation)
                            .ok_or(Error::UnreachableProvider(provider))?
                    };
                    DefaultTargetIdentityQueriesV1::new(provider, foundation, self.identities)
                        .validate_nested_callable_identity(
                            occurrence.descriptor(),
                            origin,
                            self.meter,
                            &path,
                        )
                        .map_err(|source| Error::Occurrence {
                            site: occurrence.site(),
                            source: Box::new(source),
                        })?;
                }
                Ok(())
            };
            validate().map_err(|source| Error::Template {
                index,
                key: template.key(),
                source: Box::new(source),
            })?;
        }
        Ok(())
    }
}
