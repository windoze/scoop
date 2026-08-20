//! Binds actual nested descriptors to their provider's shared artifact keys.

use scoop_hir::{ExportDefaultCallableTargetV1, validate_default_nested_callable_identity};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultNestedIdentityError;
type Error = CrossConeHirDefaultNestedIdentityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_nested_identities(&mut self) -> Result<(), Error> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();

        for (index, template) in templates.iter().enumerate() {
            let path = path.clone().index(index as u64);
            let validate = || -> Result<(), Error> {
                let nested = template.index_nested_callables(&path)?;
                for reference in template.references().callables() {
                    if let ExportDefaultCallableTargetV1::LocalFunction { declaration } =
                        reference.target()
                    {
                        nested
                            .require_local_declaration(*declaration)
                            .map_err(Error::Reference)?;
                    }
                }
                for occurrence in nested.occurrences() {
                    let origin = occurrence.definition_origin();
                    let provider = origin.origin().source().cone();

                    let foundation = if provider == self.current {
                        self.current_foundation
                    } else {
                        self.dependencies
                            .iter()
                            .find(|entry| entry.identity == provider)
                            .map(|entry| entry.foundation)
                            .ok_or(Error::UnreachableProvider(provider))?
                    };
                    validate_default_nested_callable_identity(
                        foundation,
                        provider,
                        occurrence.descriptor(),
                        origin,
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
