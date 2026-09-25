use super::*;

pub(super) fn validate(
    current: &Declarations<'_, '_, '_>,
    dependencies: &[&Declarations<'_, '_, '_>],
) -> Result<(), Error> {
    let mut previous = None;
    for dependency in dependencies {
        let provider = dependency.provider();
        if provider == current.provider() || previous.is_some_and(|last| last >= provider) {
            return Err(Error::DependencyOrder(provider));
        }
        if !std::ptr::eq(
            current.foundation.identities,
            dependency.foundation.identities,
        ) {
            return Err(Error::IdentityGraph(provider));
        }
        previous = Some(provider);
    }
    Ok(())
}
impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn provider(
        &self,
        provider: ConeIdentity,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        if provider == self.current.provider() {
            return Ok(self.current);
        }

        self.dependencies
            .binary_search_by_key(&provider, |source| source.provider())
            .map(|index| self.dependencies[index])
            .map_err(|_| Error::MissingProvider(provider))
    }
    pub(super) fn nominal_domain(
        &self,
        owner: SourceNominalId,
        arity: usize,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let provider = self.provider(self.nominal_provider(owner)?)?;
        let key = provider.source_key(subject(owner)).map_err(Error::access)?;
        if !matches!(
            key.declaration_kind(),
            SourceDeclarationKind::Class
                | SourceDeclarationKind::Interface
                | SourceDeclarationKind::Struct
                | SourceDeclarationKind::Enum
                | SourceDeclarationKind::Object
        ) {
            return Err(Error::NominalKind(owner));
        }
        let expected = key.duplicate_signature().type_parameter_count();
        if usize::try_from(expected).ok() != Some(arity) {
            return Err(Error::Arity {
                owner,
                expected,
                actual: arity,
            });
        }
        provider
            .source_lookup_domain(subject(owner))
            .map_err(Error::domain)
    }
}
