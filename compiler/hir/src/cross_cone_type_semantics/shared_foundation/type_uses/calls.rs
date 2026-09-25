use super::*;

mod receivers;
mod signatures;

impl Graph<'_> {
    pub(super) fn calls(&mut self, meter: &mut BudgetMeter) -> Result<(), Error> {
        let path = WirePath::root().field(10);
        for (reference_index, reference) in self
            .current
            .public
            .external_references()
            .records()
            .iter()
            .enumerate()
        {
            for (call_index, call) in reference.call_sites().records().iter().enumerate() {
                let path = path
                    .clone()
                    .index(reference_index as u64)
                    .field(5)
                    .index(call_index as u64);
                meter.charge_work(1 + u64::from(self.providers.len().max(1).ilog2()), &path)?;
                let provider = self
                    .providers
                    .get(&reference.origin())
                    .ok_or(Error::MissingProvider(reference.origin()))?;
                match call.reason() {
                    crate::HirDependencyCallReasonV1::SourceBinding(_) => {
                        let metadata = provider.metadata;
                        let source = call
                            .validate_source_signature(reference.target(), metadata, meter, &path)
                            .map_err(|source| Error::CallSignature {
                                position: call.position(),
                                source: Box::new(source),
                            })?;
                        self.source_receiver(call.receiver(), meter)?;
                        self.call_signature(source, meter, &path)?;
                        self.source_construction(source, meter)?;
                    }
                    crate::HirDependencyCallReasonV1::CastFailure { .. } => {
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
            }
        }
        Ok(())
    }
}
