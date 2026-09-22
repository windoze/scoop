use super::*;

pub(super) fn validate(
    current: &Declarations<'_, '_, '_>,
    dependencies: &[&Declarations<'_, '_, '_>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_table_entries(dependencies.len() as u64, &path)?;
    meter.charge_work((dependencies.len() as u64 + 1).saturating_mul(65), &path)?;
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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        meter.charge_work(65, path)?;
        if provider == self.current.provider() {
            return Ok(self.current);
        }
        meter.charge_work(
            (u64::from(self.dependencies.len().max(1).ilog2()) + 1) * 65,
            path,
        )?;
        self.dependencies
            .binary_search_by_key(&provider, |source| source.provider())
            .map(|index| self.dependencies[index])
            .map_err(|_| Error::MissingProvider(provider))
    }
    pub(super) fn nominal_domain(
        &self,
        owner: SourceNominalId,
        arity: usize,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let provider = self.provider(self.nominal_provider(owner, meter, path)?, meter, path)?;
        let key = provider
            .source_key(subject(owner), meter)
            .map_err(Error::access)?;
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
            .source_lookup_domain(subject(owner), meter)
            .map_err(Error::domain)
    }
}
