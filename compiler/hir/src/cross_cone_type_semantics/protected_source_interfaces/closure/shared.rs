//! Join the existing restricted protocol projection to shared source metadata.

use super::*;
use crate::{CrossConeTypeSemanticsSectionV1, SharedTypeMetadataError, SharedTypeMetadataV1};
use std::convert::Infallible;

type Error = SharedTypeMetadataError;

impl CanonicalProtectedCallableSourceInterfacesV1 {
    pub(in crate::cross_cone_type_semantics) fn validate_shared_declarations(
        &self,
        section: &CrossConeTypeSemanticsSectionV1,
        metadata: SharedTypeMetadataV1<'_>,
    ) -> Result<(), Error> {
        let sources =
            collect::sources::<Infallible>(section.protected_declarations(), section.inheritance())
                .map_err(source_error)?;
        let path = WirePath::root();

        if !sources.iter().map(source::Source::owner).eq(self
            .records()
            .iter()
            .map(ProtectedCallableSourceInterfaceV1::owner))
        {
            return Err(Error::SourceProtocolInventory(metadata.provider));
        }
        let shared = metadata.public.source_interfaces();
        for (index, record) in self.records().iter().enumerate() {
            let at = path.clone().index(index as u64);

            let expected = shared
                .get(record.owner())
                .ok_or(Error::SourceProtocolContract(record.owner()))?;
            if !record.matches_shared_interface(expected, &at)? {
                return Err(Error::SourceProtocolContract(record.owner()));
            }
        }
        self.validate_default_closure(section.protected_defaults().keys())
            .map_err(|error| match error {
                ProtectedSourceIndexError::Resource(error) => Error::Resource(error),
                error => source_error(ProtectedSourceClosureError::DefaultClosure(error)),
            })
    }
}

fn source_error(error: ProtectedSourceClosureError<Infallible>) -> Error {
    match error {
        ProtectedSourceClosureError::Resource(error) => Error::Resource(error),
        error => Error::SourceProtocols(Box::new(error)),
    }
}
