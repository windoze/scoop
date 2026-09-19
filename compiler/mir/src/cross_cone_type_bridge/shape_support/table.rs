use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirShapeSupportsV1 {
    provider: ConeIdentity,
    records: Vec<ParamFreeMirShapeSupportV1>,
}
impl CanonicalMirShapeSupportsV1 {
    pub fn try_new(
        provider: ConeIdentity,
        authority: MirShapeSupportAuthority<'_>,
        mut records: Vec<ParamFreeMirShapeSupportV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirShapeSupportError> {
        check_provider(provider, &records, meter)?;
        for _ in 0..usize::BITS - records.len().max(1).saturating_sub(1).leading_zeros() {
            meter.charge_work(records.len() as u64, &WirePath::root())?;
        }
        records.sort_unstable_by_key(ParamFreeMirShapeSupportV1::source);
        for (index, record) in records.iter().enumerate() {
            if index > 0 && records[index - 1].source() == record.source() {
                return Err(MirShapeSupportError::DuplicateSource {
                    source: record.source(),
                });
            }
            authority.validate(record, meter)?;
        }
        Ok(Self { provider, records })
    }
    pub fn records(&self) -> &[ParamFreeMirShapeSupportV1] {
        &self.records
    }
    pub fn get(&self, source: PersistentTypeId) -> Option<&ParamFreeMirShapeSupportV1> {
        self.records
            .binary_search_by_key(&source, ParamFreeMirShapeSupportV1::source)
            .ok()
            .map(|index| &self.records[index])
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    /// Checks exact coverage of an independently established source set.
    /// This operation does not itself prove source eligibility.
    pub fn validate_required_sources(
        &self,
        required: &[PersistentTypeId],
        meter: &mut BudgetMeter,
    ) -> Result<(), MirShapeSupportError> {
        let path = WirePath::root();
        meter.check_table_entries(required.len() as u64, &path)?;
        meter.charge_work(required.len() as u64, &path)?;
        if let Some(index) = required.windows(2).position(|pair| pair[0] >= pair[1]) {
            return Err(MirShapeSupportError::NonCanonicalRequiredSources { index: index + 1 });
        }
        if self.provider == ConeIdentity::CORE && !required.is_empty() {
            return Err(MirShapeSupportError::CoreUsesExistingAuthority);
        }
        let mut actual = self.records.iter().peekable();
        for source in required {
            meter.charge_work(1, &path)?;
            match actual.peek() {
                Some(record) if record.source() == *source => {
                    actual.next();
                }
                Some(record) if record.source() < *source => {
                    return Err(MirShapeSupportError::UnexpectedSource {
                        source: record.source(),
                    });
                }
                _ => return Err(MirShapeSupportError::MissingSource { source: *source }),
            }
        }
        if let Some(record) = actual.next() {
            return Err(MirShapeSupportError::UnexpectedSource {
                source: record.source(),
            });
        }
        Ok(())
    }
}

fn check_provider(
    provider: ConeIdentity,
    records: &[ParamFreeMirShapeSupportV1],
    meter: &mut BudgetMeter,
) -> Result<(), MirShapeSupportError> {
    meter.check_table_entries(records.len() as u64, &WirePath::root())?;
    meter.charge_work(records.len() as u64, &WirePath::root())?;
    if provider == ConeIdentity::CORE && !records.is_empty() {
        return Err(MirShapeSupportError::CoreUsesExistingAuthority);
    }
    for record in records {
        if record.provider() != provider {
            return Err(MirShapeSupportError::ProviderMismatch {
                source: record.source(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalMirShapeSupportsV1 {
    records: Vec<DecodedParamFreeMirShapeSupportV1>,
}
impl DecodedCanonicalMirShapeSupportsV1 {
    pub fn validate(
        self,
        provider: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        types: &CanonicalParamFreeMirTypeExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalMirShapeSupportsV1, MirShapeSupportError> {
        let mut records: Vec<ParamFreeMirShapeSupportV1> = Vec::new();
        let path = WirePath::root();
        meter.check_table_entries(self.records.len() as u64, &path)?;
        if provider == ConeIdentity::CORE && !self.records.is_empty() {
            return Err(MirShapeSupportError::CoreUsesExistingAuthority);
        }
        meter.try_reserve_collection_slots(&mut records, self.records.len(), &path)?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            meter.charge_nodes(1, &path.clone().index(index as u64))?;
            let record = decoded.validate(identities, types, meter)?;
            if records
                .last()
                .is_some_and(|previous| previous.source() >= record.source())
            {
                return Err(MirShapeSupportError::NonCanonicalSourceOrder { index });
            }
            records.push(record);
        }
        check_provider(provider, &records, meter)?;
        Ok(CanonicalMirShapeSupportsV1 { provider, records })
    }
}
impl WireEncode for CanonicalMirShapeSupportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireEncode for DecodedCanonicalMirShapeSupportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalMirShapeSupportsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeMirShapeSupportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
