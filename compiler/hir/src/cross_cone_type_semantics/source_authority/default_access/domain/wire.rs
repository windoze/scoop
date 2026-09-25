use super::*;
use crate::DefaultSourceAccessResolutionError as Error;
use crate::{
    DecodedCanonicalPersistentIdsV1, DecodedPersistentAccessDomainV1, PersistentAccessResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceAccessDomainV1 {
    persistent: DecodedPersistentAccessDomainV1,
    generic_subclasses: DecodedCanonicalPersistentIdsV1<PersistentGenericTypeId>,
}
impl DecodedDefaultSourceAccessDomainV1 {
    pub fn resolve<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultSourceAccessDomainV1, Error<E>> {
        // Manually constructed decoded values require the same leaf checks as CBOR.

        let persistent = self.persistent.resolve(resolver).map_err(Error::Domain)?;
        let generic_subclasses = self
            .generic_subclasses
            .resolve(resolver)
            .map_err(Error::GenericSubclasses)?;
        DefaultSourceAccessDomainV1::try_new(persistent, generic_subclasses).map_err(Error::Build)
    }
}
impl WireDecode for DecodedDefaultSourceAccessDomainV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        Ok(Self {
            persistent: d.field(1, DecodedPersistentAccessDomainV1::decode)?,
            generic_subclasses: d.field(2, DecodedCanonicalPersistentIdsV1::decode)?,
        })
    }
}
macro_rules! encode_domain {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(2)?;
                e.field(1)?;
                self.persistent.encode(e)?;
                e.field(2)?;
                self.generic_subclasses.encode(e)
            }
        }
    };
}
encode_domain!(DefaultSourceAccessDomainV1);
encode_domain!(DecodedDefaultSourceAccessDomainV1);
