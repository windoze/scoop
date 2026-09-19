use super::*;
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encode,
    encoded_length,
};
use std::fmt;

mod errors;
pub use errors::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalSelectedExternalTypeUsesV1 {
    records: Vec<SelectedExternalTypeUseV1>,
}
impl CanonicalSelectedExternalTypeUsesV1 {
    pub fn try_new(
        records: Vec<SelectedExternalTypeUseV1>,
    ) -> Result<Self, SelectedTypeUseBuildError> {
        let mut keyed = records
            .into_iter()
            .map(|record| encode(&record).map(|key| (key, record)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(SelectedTypeUseBuildError::Encoding)?;
        keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        for (index, pair) in keyed.windows(2).enumerate() {
            if pair[0].0 == pair[1].0 {
                return Err(SelectedTypeUseBuildError::Duplicate { index: index + 1 });
            }
        }
        Ok(Self {
            records: keyed.into_iter().map(|(_, record)| record).collect(),
        })
    }
    pub fn records(&self) -> &[SelectedExternalTypeUseV1] {
        &self.records
    }
}
impl WireEncode for CanonicalSelectedExternalTypeUsesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalSelectedExternalTypeUsesV1 {
    records: Vec<DecodedSelectedExternalTypeUseV1>,
}
impl DecodedCanonicalSelectedExternalTypeUsesV1 {
    pub fn resolve<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalSelectedExternalTypeUsesV1, SelectedTypeUseResolutionError<E>> {
        use SelectedTypeUseResolutionError as Error;
        meter
            .check_table_entries(self.records.len() as u64, path)
            .map_err(Error::Resource)?;
        meter
            .charge_work(self.records.len() as u64, path)
            .map_err(Error::Resource)?;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), path)
            .map_err(Error::Resource)?;
        let mut previous: Option<Vec<u8>> = None;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let at = path.clone().index(index as u64);
            let bytes = encoded_length(&decoded).map_err(Error::Encoding)?;
            meter
                .charge_owned_bytes(bytes, &at)
                .map_err(Error::Resource)?;
            meter.charge_work(bytes, &at).map_err(Error::Resource)?;
            let record = decoded.resolve(resolver, meter, &at)?;
            let key = encode(&record).map_err(Error::Encoding)?;
            if let Some(previous) = &previous {
                meter
                    .charge_work(previous.len() as u64 + key.len() as u64, &at)
                    .map_err(Error::Resource)?;
                match previous.cmp(&key) {
                    std::cmp::Ordering::Equal => return Err(Error::Duplicate { index }),
                    std::cmp::Ordering::Greater => return Err(Error::NonCanonicalOrder { index }),
                    std::cmp::Ordering::Less => {}
                }
            }
            previous = Some(key);
            records.push(record);
        }
        Ok(CanonicalSelectedExternalTypeUsesV1 { records })
    }
}
impl WireEncode for DecodedCanonicalSelectedExternalTypeUsesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalSelectedExternalTypeUsesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSelectedExternalTypeUseV1::decode(decoder))
            .map(|records| Self { records })
    }
}
