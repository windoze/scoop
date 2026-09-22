use super::*;
use crate::cross_cone_type_semantics::wire as shared;
use crate::{CallableDeclarationIdResolver, DecodedProtectedDefaultTemplateKeyV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedRecord {
    key: DecodedProtectedDefaultTemplateKeyV1,
    profile: ProtectedDefaultWitnessSourceProfileV1,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalDefaultSourceProfilesV1 {
    records: Vec<DecodedRecord>,
}
impl DecodedCanonicalDefaultSourceProfilesV1 {
    pub fn resolve<R: CallableDeclarationIdResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalDefaultSourceProfilesV1, SourceInventoryError> {
        meter.charge_owned_bytes(
            (self.records.len() as u64)
                .saturating_mul(std::mem::size_of::<DefaultSourceProfileV1>() as u64),
            &WirePath::root(),
        )?;
        let mut records = reserve(self.records.len(), meter)?;
        for record in self.records {
            records.push(DefaultSourceProfileV1::new(
                record.key.resolve(resolver).map_err(reference)?,
                record.profile,
            ));
        }
        CanonicalDefaultSourceProfilesV1::from_ordered(records, meter)
    }
}
impl WireDecode for DecodedCanonicalDefaultSourceProfilesV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.decode_array(|d, _| {
            d.expect_map(2)?;
            Ok(DecodedRecord {
                key: d.field(1, DecodedProtectedDefaultTemplateKeyV1::decode)?,
                profile: d.field(2, |d| {
                    d.expect_map(1)?;
                    match d.field(0, Decoder::unsigned)? {
                        1 => Ok(ProtectedDefaultWitnessSourceProfileV1::ParamFree),
                        2 => Ok(ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata),
                        tag => Err(shared::error(d, WireErrorKind::UnknownTag { tag })),
                    }
                })?,
            })
        })
        .map(|records| Self { records })
    }
}
impl WireEncode for DefaultSourceProfileV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        record(e, &self.key, self.profile)
    }
}
impl WireEncode for CanonicalDefaultSourceProfilesV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        shared::sequence(e, &self.records)
    }
}
impl WireEncode for DecodedCanonicalDefaultSourceProfilesV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.records.len() as u64)?;
        for r in &self.records {
            record(e, &r.key, r.profile)?;
        }
        Ok(())
    }
}
fn record(
    e: &mut Encoder,
    key: &impl WireEncode,
    profile: ProtectedDefaultWitnessSourceProfileV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(2)?;
    e.field(1)?;
    key.encode(e)?;
    e.field(2)?;
    e.map(1)?;
    e.field(0)?;
    e.unsigned(match profile {
        ProtectedDefaultWitnessSourceProfileV1::ParamFree => 1,
        ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata => 2,
    })
}
