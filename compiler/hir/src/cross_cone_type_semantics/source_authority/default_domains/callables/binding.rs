use super::*;
mod errors;
pub use errors::DefaultSourceCallableDomainBindingError;
type BindingError = DefaultSourceCallableDomainBindingError;

/// Complete callable target-domain equality. Receiver, operation, nested ABI,
/// profile and call-domain coverage are separate obligations.
#[derive(Debug)]
pub struct BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.declarations
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_callable_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
    ) -> Result<BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
        let path = WirePath::root();

        let foundation = declarations
            .origins()
            .parameters()
            .members()
            .nominals
            .foundation;
        if !std::ptr::eq(self.current.foundation, foundation) {
            return Err(BindingError::Foundation {
                expected: self.current.provider(),
                actual: declarations.provider(),
            });
        }

        for (index, declaration) in declarations.declarations().iter().enumerate() {
            let path = path.clone().index(index as u64);

            let shape = declaration.provider_binders();

            let scope = shape.signature_scope();
            let occurrences = declaration.references().occurrences();

            for occurrence in occurrences {
                let DefaultSourceReferenceRecordV1::Callable(reference) = occurrence.source()
                else {
                    continue;
                };

                let path = path
                    .clone()
                    .field(11)
                    .field(1)
                    .index(u64::from(occurrence.index()))
                    .field(1);
                let context = Context {
                    declaration,
                    occurrence: occurrence.body(),
                    scope: &scope,
                };
                let expected = self
                    .callable_source_domain_at(reference.target().into(), &context, &path)
                    .map_err(|error| {
                        BindingError::target(declaration.key(), occurrence.index(), error)
                    })?;
                let actual = reference.witness().target_domain();

                if actual != &expected {
                    return Err(BindingError::Witness {
                        key: declaration.key(),
                        index: occurrence.index(),
                    });
                }
            }
        }
        Ok(BoundNominalDefaultCallableDomainsV1 { declarations })
    }
}
