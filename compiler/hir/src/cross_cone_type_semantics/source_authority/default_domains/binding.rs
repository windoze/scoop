//! Joins independently replayed type domains to every bound body occurrence.
use super::*;
mod errors;
mod scope;
pub use errors::*;
type BindingError = DefaultSourceTypeDomainBindingError;

/// Complete type-reference domain equality only. Other target kinds, operation
/// typing, receiver permission, profile and call-domain coverage remain separate.
#[derive(Debug)]
pub struct BoundNominalDefaultTypeDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultTypeDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.declarations
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_type_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
    ) -> Result<BoundNominalDefaultTypeDomainsV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
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

            // The shape comes from the original provider, including inherited defaults.
            let shape = declaration.provider_binders();

            let scope = shape.signature_scope();
            let occurrences = declaration.references().occurrences();

            for occurrence in occurrences {
                let DefaultSourceReferenceRecordV1::Type(reference) = occurrence.source() else {
                    continue;
                };

                let path = path
                    .clone()
                    .field(11)
                    .field(3)
                    .index(u64::from(occurrence.index()))
                    .field(1);
                let expected = (|| {
                    let local_scope;
                    let scope = match occurrence.body().attachment {
                        DefaultBodyReferenceAttachmentV1::Metadata(
                            DefaultBodyReferenceMetadataV1::LocalFunction(function),
                        ) => {
                            local_scope = scope::local(self, function, &scope, &path)?;
                            &local_scope
                        }
                        _ => &scope,
                    };
                    self.type_source_domain_at(reference.target(), scope, &path)
                })()
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
        Ok(BoundNominalDefaultTypeDomainsV1 { declarations })
    }
}
