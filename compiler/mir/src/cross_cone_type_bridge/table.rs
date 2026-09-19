use super::*;

/// Canonical by exact identity, preserving every representation's source order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalParamFreeMirTypeExportsV1 {
    records: Vec<ParamFreeMirTypeExportV1>,
}
impl CanonicalParamFreeMirTypeExportsV1 {
    pub fn try_new(mut records: Vec<ParamFreeMirTypeExportV1>) -> Result<Self, MirTypeBridgeError> {
        records.sort_unstable_by_key(ParamFreeMirTypeExportV1::exact);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].exact() == pair[1].exact())
        {
            return Err(MirTypeBridgeError::DuplicateType {
                exact: pair[0].exact(),
            });
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ParamFreeMirTypeExportV1] {
        &self.records
    }
    pub fn get(&self, exact: PersistentExactTypeId) -> Option<&ParamFreeMirTypeExportV1> {
        self.records
            .binary_search_by_key(&exact, ParamFreeMirTypeExportV1::exact)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalParamFreeMirTypeExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalParamFreeMirTypeExportsV1 {
    records: Vec<DecodedParamFreeMirTypeExportV1>,
}
impl DecodedCanonicalParamFreeMirTypeExportsV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &crate::OdrFreeMirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalParamFreeMirTypeExportsV1, MirTypeBridgeError> {
        let mut records = Vec::new();
        let path = WirePath::root();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &path)
            .map_err(MirTypeBridgeError::Resource)?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let path = path.clone().index(index as u64);
            meter
                .charge_nodes(1, &path)
                .map_err(MirTypeBridgeError::Resource)?;
            meter
                .charge_work(4, &path)
                .map_err(MirTypeBridgeError::Resource)?;
            let record = decoded.validate(identities, foundation, meter)?;
            if records
                .last()
                .is_some_and(|previous: &ParamFreeMirTypeExportV1| {
                    previous.exact() >= record.exact()
                })
            {
                return Err(MirTypeBridgeError::NonCanonicalTypeOrder { index });
            }
            records.push(record);
        }
        Ok(CanonicalParamFreeMirTypeExportsV1 { records })
    }
}
impl WireEncode for DecodedCanonicalParamFreeMirTypeExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalParamFreeMirTypeExportsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeMirTypeExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
