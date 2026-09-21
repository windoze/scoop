use super::*;

pub(super) struct Providers<'b, 'f> {
    current: &'b BoundTypeFoundationSourcesV1<'f>,
    dependencies: &'b [&'b BoundTypeFoundationSourcesV1<'f>],
}
impl<'b, 'f> Providers<'b, 'f> {
    pub(super) fn new(
        current: &'b BoundTypeFoundationSourcesV1<'f>,
        dependencies: &'b [&'b BoundTypeFoundationSourcesV1<'f>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_table_entries(dependencies.len() as u64, &path)?;
        meter.charge_work(dependencies.len() as u64 + 1, &path)?;
        let mut previous = None;
        for dependency in dependencies {
            let provider = dependency.source().entries().provider;
            if provider == current.source().entries().provider
                || previous.is_some_and(|previous| previous >= provider)
            {
                return Err(Error::DependencyOrder(provider));
            }
            if !std::ptr::eq(current.identities, dependency.identities) {
                return Err(Error::IdentityGraph(provider));
            }
            previous = Some(provider);
        }
        Ok(Self {
            current,
            dependencies,
        })
    }

    pub(super) fn get(
        &self,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b BoundTypeFoundationSourcesV1<'f>, Error> {
        meter.charge_work(1, path)?;
        if provider == self.current.source().entries().provider {
            return Ok(self.current);
        }
        meter.charge_work(u64::from(self.dependencies.len().max(1).ilog2()) + 1, path)?;
        self.dependencies
            .binary_search_by_key(&provider, |source| source.source().entries().provider)
            .map(|index| self.dependencies[index])
            .map_err(|_| Error::MissingProvider(provider))
    }
}
