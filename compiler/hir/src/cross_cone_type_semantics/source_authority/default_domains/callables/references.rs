use super::*;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn reference_domain(
        &self,
        reference: &DefaultCallableReferenceV1,
        context: &Context<'_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let target = reference.source_access_target().map_err(Error::target)?;
        self.callable_source_domain_at(target, context, meter, path)
    }
}
