use super::*;
mod errors;
pub use errors::DefaultSourceValueDomainBindingError;
type BindingError = DefaultSourceValueDomainBindingError;

/// Domain equality for every constructor, global, singleton and field occurrence.
/// Other reference kinds, operation typing, receivers and coverage remain separate.
#[derive(Debug)]
pub struct BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.declarations
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_value_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
    ) -> Result<BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
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

        for declaration in declarations.declarations() {
            let occurrences = declaration.references().occurrences();

            for occurrence in occurrences {
                let record = occurrence.source();
                let target = match record {
                    DefaultSourceReferenceRecordV1::Constructor(r) => {
                        Target::Constructor(r.target())
                    }
                    DefaultSourceReferenceRecordV1::Global(r) => Target::Global(*r.target()),
                    DefaultSourceReferenceRecordV1::Singleton(r) => Target::Singleton(*r.target()),
                    DefaultSourceReferenceRecordV1::Field(r) => Target::Field(r.target()),
                    DefaultSourceReferenceRecordV1::Callable(_)
                    | DefaultSourceReferenceRecordV1::Type(_) => continue,
                };
                let kind = record.kind();
                let expected = self.value_source_domain_at(target).map_err(|error| {
                    BindingError::target(declaration.key(), kind, occurrence.index(), error)
                })?;
                let actual = record.witness().target_domain();

                if actual != &expected {
                    return Err(BindingError::Witness {
                        key: declaration.key(),
                        kind,
                        index: occurrence.index(),
                    });
                }
            }
        }
        Ok(BoundNominalDefaultValueDomainsV1 { declarations })
    }
}
