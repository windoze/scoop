use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirObjectValuesV1 {
    records: Vec<ParamFreeMirObjectValueV1>,
}
impl CanonicalMirObjectValuesV1 {
    pub fn try_new(
        mut records: Vec<ParamFreeMirObjectValueV1>,
    ) -> Result<Self, MirObjectBridgeError> {
        records.sort_unstable_by_key(ParamFreeMirObjectValueV1::value);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].value() == pair[1].value())
        {
            return Err(MirObjectBridgeError::DuplicateObject {
                value: pair[0].value(),
            });
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ParamFreeMirObjectValueV1] {
        &self.records
    }
    pub fn get(&self, value: PersistentObjectValueId) -> Option<&ParamFreeMirObjectValueV1> {
        self.records
            .binary_search_by_key(&value, ParamFreeMirObjectValueV1::value)
            .ok()
            .map(|index| &self.records[index])
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalMirObjectValuesV1 {
    records: Vec<DecodedParamFreeMirObjectValueV1>,
}
impl DecodedCanonicalMirObjectValuesV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
    ) -> Result<CanonicalMirObjectValuesV1, MirObjectBridgeError> {
        let mut records: Vec<ParamFreeMirObjectValueV1> = reserve(self.records.len())?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let record = decoded.validate(graph, types, callables)?;
            if records
                .last()
                .is_some_and(|previous| previous.value() >= record.value())
            {
                return Err(MirObjectBridgeError::NonCanonicalObjectOrder { index });
            }
            records.push(record);
        }
        Ok(CanonicalMirObjectValuesV1 { records })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirExternalInitializationUsesV1 {
    records: Vec<SelectedExternalInitializationUseV1>,
}
impl CanonicalMirExternalInitializationUsesV1 {
    pub fn try_new(
        mut records: Vec<SelectedExternalInitializationUseV1>,
    ) -> Result<Self, MirObjectBridgeError> {
        records.sort_unstable();
        if records.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(MirObjectBridgeError::DuplicateInitializationUse);
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[SelectedExternalInitializationUseV1] {
        &self.records
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalMirExternalInitializationUsesV1 {
    records: Vec<DecodedSelectedExternalInitializationUseV1>,
}
impl DecodedCanonicalMirExternalInitializationUsesV1 {
    pub fn validate(
        self,
        consumer: ConeIdentity,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalMirExternalInitializationUsesV1, MirObjectBridgeError> {
        let mut records = reserve(self.records.len())?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let record = decoded.validate(consumer, graph)?;
            if records.last().is_some_and(|previous| *previous >= record) {
                return Err(MirObjectBridgeError::NonCanonicalInitializationUseOrder { index });
            }
            records.push(record);
        }
        Ok(CanonicalMirExternalInitializationUsesV1 { records })
    }
}
macro_rules! encode_table {
    ($($ty:ty),+) => { $(impl WireEncode for $ty {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> { sequence(encoder, &self.records) }
    })+ };
}
encode_table!(
    CanonicalMirObjectValuesV1,
    DecodedCanonicalMirObjectValuesV1,
    CanonicalMirExternalInitializationUsesV1,
    DecodedCanonicalMirExternalInitializationUsesV1
);
impl WireDecode for DecodedCanonicalMirObjectValuesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeMirObjectValueV1::decode(decoder))
            .map(|records| Self { records })
    }
}
impl WireDecode for DecodedCanonicalMirExternalInitializationUsesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSelectedExternalInitializationUseV1::decode(decoder))
            .map(|records| Self { records })
    }
}
fn reserve<T>(count: usize) -> Result<Vec<T>, WireError> {
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, count, &WirePath::root())?;
    Ok(records)
}
