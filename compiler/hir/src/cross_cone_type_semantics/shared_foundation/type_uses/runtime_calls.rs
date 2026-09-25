use super::*;

impl Graph<'_> {
    pub(super) fn runtime_calls(&mut self, meter: &mut BudgetMeter) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        for reference in self.current.public.external_references().records() {
            for call in reference.call_sites().records() {
                meter.charge_work(1 + u64::from(self.providers.len().max(1).ilog2()), &path)?;
                if !matches!(
                    call.reason(),
                    crate::HirDependencyCallReasonV1::CastFailure { .. }
                ) {
                    continue;
                }
                let provider = self
                    .providers
                    .get(&reference.origin())
                    .ok_or(Error::MissingProvider(reference.origin()))?;
                let (constructor, owner) = call
                    .runtime_constructor_source(
                        reference.target(),
                        reference.origin(),
                        provider.metadata.identities,
                        provider.metadata.public,
                        meter,
                    )
                    .map_err(|error| Error::RuntimeConstructor(Box::new(error)))?;
                self.select(owner, Kind::Construct(constructor), meter)?;
                self.select(owner, Kind::Signature, meter)?;
            }
        }
        Ok(())
    }
}
