use super::*;
use crate::DefaultSourceAccessResolutionError as Error;
use crate::{
    DecodedCanonicalPersistentIdsV1, DecodedPersistentAccessConstraintV1,
    DecodedPersistentAccessDomainV1, PersistentAccessResolver,
};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceAccessDomainV1 {
    persistent: DecodedPersistentAccessDomainV1,
    generic_subclasses: DecodedCanonicalPersistentIdsV1<PersistentGenericTypeId>,
}
impl DecodedDefaultSourceAccessDomainV1 {
    pub fn resolve<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceAccessDomainV1, Error<E>> {
        let path = WirePath::root();
        meter
            .check_semantic_depth(2, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        // Manually constructed decoded values require the same leaf checks as CBOR.
        if let DecodedPersistentAccessDomainV1::Conjunction(constraints) = &self.persistent {
            meter
                .check_table_entries(constraints.len() as u64, &path)
                .map_err(Error::Resource)?;
            meter
                .charge_work(constraints.len() as u64, &path)
                .map_err(Error::Resource)?;
            for constraint in constraints {
                if let DecodedPersistentAccessConstraintV1::File(source) = constraint {
                    meter
                        .check_semantic_leaf(source.logical_path_byte_len() as u64, &path)
                        .map_err(Error::Resource)?;
                }
            }
        }
        self.generic_subclasses
            .charge_resolution_at(meter, &path.field(2))
            .map_err(Error::Resource)?;
        let persistent = self
            .persistent
            .resolve_metered(resolver, meter)
            .map_err(Error::Domain)?;
        let generic_subclasses = self
            .generic_subclasses
            .resolve(resolver)
            .map_err(Error::GenericSubclasses)?;
        DefaultSourceAccessDomainV1::try_new(persistent, generic_subclasses).map_err(Error::Build)
    }
}
impl WireDecode for DecodedDefaultSourceAccessDomainV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
