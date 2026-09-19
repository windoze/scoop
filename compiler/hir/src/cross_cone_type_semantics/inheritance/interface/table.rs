use super::*;
use scoop_wire::{BudgetMeter, Decoder, WireDecode, WireError, WirePath};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalInheritanceInterfacesV1 {
    records: Vec<NominalInheritanceInterfaceV1>,
}
impl CanonicalNominalInheritanceInterfacesV1 {
    pub fn try_new(
        mut records: Vec<NominalInheritanceInterfaceV1>,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        records.sort_unstable_by_key(NominalInheritanceInterfaceV1::owner);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<NominalInheritanceInterfaceV1>,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        if records
            .windows(2)
            .any(|pair| pair[0].owner() >= pair[1].owner())
        {
            return Err(InheritanceInterfaceBuildError::OwnerOrder);
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalInheritanceInterfaceV1] {
        &self.records
    }
    pub fn get(&self, owner: PersistentExactTypeId) -> Option<&NominalInheritanceInterfaceV1> {
        self.records
            .binary_search_by_key(&owner, NominalInheritanceInterfaceV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalNominalInheritanceInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalInheritanceInterfacesV1 {
    records: Vec<DecodedNominalInheritanceInterfaceV1>,
}
impl DecodedCanonicalNominalInheritanceInterfacesV1 {
    pub fn resolve<R: NominalInheritanceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalNominalInheritanceInterfacesV1, InheritanceInterfaceResolutionError<E>>
    {
        use InheritanceInterfaceResolutionError as Error;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(self.records.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter)?);
        }
        CanonicalNominalInheritanceInterfacesV1::from_ordered(records).map_err(Error::Build)
    }
}
impl WireDecode for DecodedCanonicalNominalInheritanceInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedNominalInheritanceInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalNominalInheritanceInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
