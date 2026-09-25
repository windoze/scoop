use super::*;

mod members;
mod receivers;
mod signatures;

impl Graph<'_> {
    pub(super) fn calls(&mut self) -> Result<(), Error> {
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

                let provider = self
                    .providers
                    .get(&reference.origin())
                    .ok_or(Error::MissingProvider(reference.origin()))?;
                match call.reason() {
                    crate::HirDependencyCallReasonV1::SourceBinding(_) => {
                        let metadata = provider.metadata;
                        let source = call
                            .validate_source_signature(reference.target(), metadata)
                            .map_err(|source| Error::CallSignature {
                                position: call.position(),
                                source: Box::new(source),
                            })?;
                        self.source_receiver(call.receiver())?;
                        self.call_signature(source)?;
                        self.source_extension(source, metadata, call, &path)?;
                        self.source_construction(source)?;
                        self.source_member(source, metadata, call, &path)?;
                    }
                    crate::HirDependencyCallReasonV1::CastFailure { .. } => {
                        let (constructor, owner) = call
                            .runtime_constructor_source(
                                reference.target(),
                                reference.origin(),
                                provider.metadata.identities,
                                provider.metadata.public,
                            )
                            .map_err(|error| Error::RuntimeConstructor(Box::new(error)))?;
                        self.select(owner, Kind::Construct(constructor))?;
                        self.select(owner, Kind::Signature)?;
                    }
                }
            }
        }
        Ok(())
    }
}
