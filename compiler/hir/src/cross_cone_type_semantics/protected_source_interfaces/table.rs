use super::*;
use crate::ProtectedCallableInterfaceResolver;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalProtectedCallableSourceInterfacesV1 {
    records: Vec<ProtectedCallableSourceInterfaceV1>,
}
impl CanonicalProtectedCallableSourceInterfacesV1 {
    pub fn try_new(
        mut records: Vec<ProtectedCallableSourceInterfaceV1>,
    ) -> Result<Self, ProtectedSourceBuildError> {
        records.sort_unstable_by_key(ProtectedCallableSourceInterfaceV1::owner);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<ProtectedCallableSourceInterfaceV1>,
    ) -> Result<Self, ProtectedSourceBuildError> {
        for pair in records.windows(2) {
            match pair[0].owner().cmp(&pair[1].owner()) {
                std::cmp::Ordering::Equal => return Err(ProtectedSourceBuildError::DuplicateOwner),
                std::cmp::Ordering::Greater => {
                    return Err(ProtectedSourceBuildError::NonCanonicalOrder);
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ProtectedCallableSourceInterfaceV1] {
        &self.records
    }
    pub fn get(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Option<&ProtectedCallableSourceInterfaceV1> {
        self.records
            .binary_search_by_key(&owner, ProtectedCallableSourceInterfaceV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
    pub fn validate_default_closure(
        &self,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<(), ProtectedSourceIndexError> {
        let mut seen = BTreeSet::new();
        for record in &self.records {
            for parameter in record.parameters().parameters() {
                if let Some(key) = parameter.calling().template() {
                    if !seen.insert(key) {
                        return Err(ProtectedSourceIndexError::Build(
                            ProtectedSourceBuildError::DuplicateDefault,
                        ));
                    }
                }
            }
        }

        if !seen.iter().copied().eq(keys.keys().iter().copied()) {
            return Err(ProtectedSourceIndexError::Build(
                ProtectedSourceBuildError::DefaultClosure,
            ));
        }
        Ok(())
    }
    pub fn index_templates<'a>(
        &'a self,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<IndexedCanonicalProtectedCallableSourceInterfacesV1<'a>, ProtectedSourceIndexError>
    {
        self.validate_default_closure(keys)?;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &WirePath::root())
            .map_err(ProtectedSourceIndexError::Resource)?;
        for record in &self.records {
            records.push(record.index_templates(keys)?);
        }
        Ok(IndexedCanonicalProtectedCallableSourceInterfacesV1 { records })
    }
}
pub struct IndexedCanonicalProtectedCallableSourceInterfacesV1<'a> {
    records: Vec<IndexedProtectedCallableSourceInterfaceV1<'a>>,
}
impl WireEncode for IndexedCanonicalProtectedCallableSourceInterfacesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedCallableSourceInterfacesV1 {
    records: Vec<DecodedProtectedCallableSourceInterfaceV1>,
}
impl DecodedCanonicalProtectedCallableSourceInterfacesV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<CanonicalProtectedCallableSourceInterfacesV1, ProtectedSourceResolutionError<E>>
    {
        use ProtectedSourceResolutionError as Error;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &WirePath::root())
            .map_err(Error::Resource)?;
        for record in self.records {
            records.push(record.resolve(resolver, keys)?);
        }
        let table = CanonicalProtectedCallableSourceInterfacesV1::from_ordered(records)
            .map_err(Error::Build)?;
        table
            .validate_default_closure(keys)
            .map_err(|error| match error {
                ProtectedSourceIndexError::Resource(e) => Error::Resource(e),
                ProtectedSourceIndexError::Build(e) => Error::Build(e),
            })?;
        Ok(table)
    }
}
impl WireDecode for DecodedCanonicalProtectedCallableSourceInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtectedCallableSourceInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalProtectedCallableSourceInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
