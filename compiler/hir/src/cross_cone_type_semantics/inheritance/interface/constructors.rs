use super::*;
use crate::{
    DeclaredVisibilityV1, DecodedNominalSupportConstructorInterfaceV1,
    NominalSupportConstructorInterfaceV1, ProtectedCallableInterfaceResolver, SourceNominalId,
};
use scoop_identity::PersistentConstructorId;
use scoop_wire::{BudgetMeter, Decoder, WireDecode, WireError, WirePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceConstructorInterfaceV1 {
    source: NominalSupportConstructorInterfaceV1,
}
impl InheritanceConstructorInterfaceV1 {
    pub fn try_new(
        source: NominalSupportConstructorInterfaceV1,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        if !matches!(
            source.declaration_access().declared_visibility(),
            DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
        ) {
            return Err(InheritanceInterfaceBuildError::ConstructorAccess);
        }
        let payload = source.payload();
        if !matches!(payload.owner(), SourceNominalId::Concrete(_))
            || source
                .declaration_access()
                .lexical_owners()
                .iter()
                .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)))
            || !payload.type_parameters().is_empty()
            || payload.result().contains_binder()
            || payload
                .parameters()
                .parameters()
                .iter()
                .any(|parameter| parameter.value_type().contains_binder())
        {
            return Err(InheritanceInterfaceBuildError::ConstructorGeneric);
        }
        Ok(Self { source })
    }
    pub const fn declaration(&self) -> PersistentConstructorId {
        self.source.declaration()
    }
    pub const fn source(&self) -> &NominalSupportConstructorInterfaceV1 {
        &self.source
    }
}
impl WireEncode for InheritanceConstructorInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.source.encode(encoder)
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceConstructorsV1 {
    records: Vec<InheritanceConstructorInterfaceV1>,
}
impl CanonicalInheritanceConstructorsV1 {
    pub fn try_new(
        mut records: Vec<InheritanceConstructorInterfaceV1>,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        records.sort_unstable_by_key(InheritanceConstructorInterfaceV1::declaration);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<InheritanceConstructorInterfaceV1>,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        if records
            .windows(2)
            .any(|pair| pair[0].declaration() >= pair[1].declaration())
        {
            return Err(InheritanceInterfaceBuildError::ConstructorOrder);
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InheritanceConstructorInterfaceV1] {
        &self.records
    }
    pub fn get(&self, id: PersistentConstructorId) -> Option<&InheritanceConstructorInterfaceV1> {
        self.records
            .binary_search_by_key(&id, InheritanceConstructorInterfaceV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalInheritanceConstructorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceConstructorInterfaceV1(DecodedNominalSupportConstructorInterfaceV1);
impl DecodedInheritanceConstructorInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<InheritanceConstructorInterfaceV1, InheritanceInterfaceResolutionError<E>> {
        use InheritanceInterfaceResolutionError as Error;
        let source = self
            .0
            .resolve(resolver, meter)
            .map_err(Error::Constructor)?;
        meter
            .charge_work(
                scoop_wire::encoded_length(&source).map_err(Error::Encoding)?,
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        InheritanceConstructorInterfaceV1::try_new(source).map_err(Error::Build)
    }
}
impl WireDecode for DecodedInheritanceConstructorInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedNominalSupportConstructorInterfaceV1::decode(decoder).map(Self)
    }
}
impl WireEncode for DecodedInheritanceConstructorInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceConstructorsV1 {
    records: Vec<DecodedInheritanceConstructorInterfaceV1>,
}
impl DecodedCanonicalInheritanceConstructorsV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceConstructorsV1, InheritanceInterfaceResolutionError<E>> {
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
        CanonicalInheritanceConstructorsV1::from_ordered(records).map_err(Error::Build)
    }
}
impl WireDecode for DecodedCanonicalInheritanceConstructorsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedInheritanceConstructorInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalInheritanceConstructorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
