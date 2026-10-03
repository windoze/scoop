use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalHirDependencyCallSitesV1 {
    records: Vec<HirDependencyCallSiteV1>,
}

impl CanonicalHirDependencyCallSitesV1 {
    pub fn try_new(
        mut records: Vec<HirDependencyCallSiteV1>,
    ) -> Result<Self, HirDependencyCallSiteBuildError> {
        records.sort_unstable_by_key(HirDependencyCallSiteV1::position);
        Self::from_canonical(records)
    }

    fn from_canonical(
        records: Vec<HirDependencyCallSiteV1>,
    ) -> Result<Self, HirDependencyCallSiteBuildError> {
        for (index, pair) in records.windows(2).enumerate() {
            match pair[0].position().cmp(&pair[1].position()) {
                std::cmp::Ordering::Equal => {
                    return Err(HirDependencyCallSiteBuildError::DuplicatePosition(
                        pair[1].position(),
                    ));
                }
                std::cmp::Ordering::Greater => {
                    return Err(HirDependencyCallSiteBuildError::PositionOrder {
                        index: index + 1,
                    });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[HirDependencyCallSiteV1] {
        &self.records
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl WireEncode for CanonicalHirDependencyCallSitesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalHirDependencyCallSitesV1 {
    records: Vec<DecodedHirDependencyCallSiteV1>,
}

impl DecodedCanonicalHirDependencyCallSitesV1 {
    pub fn resolve<R: HirDependencyCallSiteResolver<E>, E>(
        self,
        resolver: &mut R,

        path: &WirePath,
    ) -> Result<CanonicalHirDependencyCallSitesV1, HirDependencyCallSiteResolutionError<E>> {
        use HirDependencyCallSiteResolutionError as Error;
        let mut records = Vec::new();

        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.into_iter().enumerate() {
            records.push(record.resolve(resolver, &path.clone().index(index as u64))?);
        }

        CanonicalHirDependencyCallSitesV1::from_canonical(records).map_err(Error::Shape)
    }
}

impl WireEncode for DecodedCanonicalHirDependencyCallSitesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalHirDependencyCallSitesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedHirDependencyCallSiteV1::decode(d))
            .map(|records| Self { records })
    }
}
