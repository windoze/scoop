use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;
use crate::HirDependencyCallSiteResolver;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalHirDependencyTypeSitesV1 {
    records: Vec<HirDependencyTypeSiteV1>,
}

impl CanonicalHirDependencyTypeSitesV1 {
    pub fn try_new(
        mut records: Vec<HirDependencyTypeSiteV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, HirDependencyTypeSiteBuildError> {
        let count = records.len() as u64;
        meter.check_table_entries(count, &WirePath::root())?;
        meter.charge_work(
            count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
            &WirePath::root(),
        )?;
        records.sort_unstable_by_key(HirDependencyTypeSiteV1::sort_key);
        Self::from_canonical(records)
    }

    fn from_canonical(
        records: Vec<HirDependencyTypeSiteV1>,
    ) -> Result<Self, HirDependencyTypeSiteBuildError> {
        for (index, pair) in records.windows(2).enumerate() {
            match pair[0].sort_key().cmp(&pair[1].sort_key()) {
                std::cmp::Ordering::Equal => {
                    return Err(HirDependencyTypeSiteBuildError::DuplicatePosition(
                        pair[1].position(),
                        pair[1].role(),
                    ));
                }
                std::cmp::Ordering::Greater => {
                    return Err(HirDependencyTypeSiteBuildError::PositionOrder {
                        index: index + 1,
                    });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[HirDependencyTypeSiteV1] {
        &self.records
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl WireEncode for CanonicalHirDependencyTypeSitesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalHirDependencyTypeSitesV1 {
    records: Vec<DecodedHirDependencyTypeSiteV1>,
}

impl DecodedCanonicalHirDependencyTypeSitesV1 {
    pub fn resolve<R: HirDependencyCallSiteResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalHirDependencyTypeSitesV1, HirDependencyTypeSiteResolutionError<E>> {
        let mut records = Vec::new();
        meter.charge_owned_bytes(
            (self.records.len() * std::mem::size_of::<HirDependencyTypeSiteV1>()) as u64,
            path,
        )?;
        meter.try_reserve_collection_slots(&mut records, self.records.len(), path)?;
        for (index, record) in self.records.into_iter().enumerate() {
            records.push(record.resolve(resolver, meter, &path.clone().index(index as u64))?);
        }
        meter.charge_work(records.len() as u64, path)?;
        CanonicalHirDependencyTypeSitesV1::from_canonical(records)
            .map_err(HirDependencyTypeSiteResolutionError::Shape)
    }
}

impl WireEncode for DecodedCanonicalHirDependencyTypeSitesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalHirDependencyTypeSitesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedHirDependencyTypeSiteV1::decode(d))
            .map(|records| Self { records })
    }
}
