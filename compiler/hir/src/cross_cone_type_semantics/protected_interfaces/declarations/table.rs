use super::*;
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalProtectedDeclarationRefsV1 {
    values: Vec<ProtectedDeclarationRefV1>,
}
impl CanonicalProtectedDeclarationRefsV1 {
    pub fn try_new(
        mut values: Vec<ProtectedDeclarationRefV1>,
    ) -> Result<Self, ProtectedDeclarationTableError> {
        values.sort_unstable();
        Self::from_ordered(values)
    }
    fn from_ordered(
        values: Vec<ProtectedDeclarationRefV1>,
    ) -> Result<Self, ProtectedDeclarationTableError> {
        validate_order(values.iter().copied())?;
        Ok(Self { values })
    }
    pub fn values(&self) -> &[ProtectedDeclarationRefV1] {
        &self.values
    }
}
impl WireEncode for CanonicalProtectedDeclarationRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalProtectedDeclarationInterfacesV1 {
    records: Vec<ProtectedDeclarationInterfaceV1>,
}
impl CanonicalProtectedDeclarationInterfacesV1 {
    pub fn try_new(
        mut records: Vec<ProtectedDeclarationInterfaceV1>,
    ) -> Result<Self, ProtectedDeclarationTableError> {
        records.sort_unstable_by_key(ProtectedDeclarationInterfaceV1::reference);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<ProtectedDeclarationInterfaceV1>,
    ) -> Result<Self, ProtectedDeclarationTableError> {
        validate_order(
            records
                .iter()
                .map(ProtectedDeclarationInterfaceV1::reference),
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ProtectedDeclarationInterfaceV1] {
        &self.records
    }
    pub fn get(
        &self,
        reference: ProtectedDeclarationRefV1,
    ) -> Option<&ProtectedDeclarationInterfaceV1> {
        self.records
            .binary_search_by_key(&reference, ProtectedDeclarationInterfaceV1::reference)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalProtectedDeclarationInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedDeclarationInterfacesV1 {
    records: Vec<DecodedProtectedDeclarationInterfaceV1>,
}
impl DecodedCanonicalProtectedDeclarationInterfacesV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalProtectedDeclarationInterfacesV1, ProtectedDeclarationResolutionError<E>>
    {
        use ProtectedDeclarationResolutionError as Error;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(
                (self.records.len() as u64).saturating_mul(128),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter)?);
        }
        CanonicalProtectedDeclarationInterfacesV1::from_ordered(records).map_err(Error::Table)
    }
}
impl WireDecode for DecodedCanonicalProtectedDeclarationInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtectedDeclarationInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalProtectedDeclarationInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedDeclarationRefsV1 {
    values: Vec<DecodedProtectedDeclarationRefV1>,
}
impl DecodedCanonicalProtectedDeclarationRefsV1 {
    pub(crate) fn charge_resolution_at(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        meter.check_semantic_depth(1, path)?;
        meter.check_table_entries(self.values.len() as u64, path)?;
        meter.charge_nodes(self.values.len() as u64, path)?;
        meter.charge_work((self.values.len() as u64).saturating_mul(128), path)
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalProtectedDeclarationRefsV1, ProtectedDeclarationResolutionError<E>>
    where
        R: crate::CallableDeclarationIdResolver<E>
            + crate::SignatureTypeReferenceResolver<E>
            + scoop_identity::PersistentIdResolver<scoop_identity::PersistentPropertyId, Error = E>,
    {
        use ProtectedDeclarationResolutionError as Error;
        let mut values = Vec::new();
        meter
            .try_reserve_collection_slots(&mut values, self.values.len(), &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(
                (self.values.len() as u64).saturating_mul(128),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        for value in self.values {
            values.push(value.resolve(resolver)?);
        }
        CanonicalProtectedDeclarationRefsV1::from_ordered(values).map_err(Error::Table)
    }
}
impl WireDecode for DecodedCanonicalProtectedDeclarationRefsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtectedDeclarationRefV1::decode(d))
            .map(|values| Self { values })
    }
}
impl WireEncode for DecodedCanonicalProtectedDeclarationRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}
fn validate_order(
    values: impl IntoIterator<Item = ProtectedDeclarationRefV1>,
) -> Result<(), ProtectedDeclarationTableError> {
    let mut previous = None;
    for value in values {
        if let Some(previous) = previous {
            match value.cmp(&previous) {
                std::cmp::Ordering::Equal => return Err(ProtectedDeclarationTableError::Duplicate),
                std::cmp::Ordering::Less => {
                    return Err(ProtectedDeclarationTableError::NonCanonicalOrder);
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        previous = Some(value);
    }
    Ok(())
}
