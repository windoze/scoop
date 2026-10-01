use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

use super::*;

#[derive(Debug)]
pub struct DecodedCanonicalExactLayoutExportsV1 {
    records: Vec<DecodedExactLayoutExportV1>,
}

impl DecodedCanonicalExactLayoutExportsV1 {
    pub fn read_link(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
        dependencies: &[&CanonicalExactLayoutExportsV1],
    ) -> Result<CanonicalExactLayoutExportsV1, crate::LinkDataError> {
        use crate::link_data::link_error;
        let mut available = std::collections::BTreeMap::new();
        for dependency in dependencies {
            for record in dependency.records() {
                available
                    .entry(record.identity().layout())
                    .or_insert_with(|| record.clone());
            }
        }
        let mut pending = self
            .records
            .into_iter()
            .map(|record| {
                let id = record.link_id(identities)?;
                let dependencies = record.link_dependencies(target, identities)?;
                Ok((id, dependencies, record))
            })
            .collect::<Result<Vec<_>, crate::LinkDataError>>()?;
        if pending.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(crate::LinkDataError(
                "noncanonical layout table order".into(),
            ));
        }
        let mut complete = Vec::new();
        while !pending.is_empty() {
            let Some(index) = pending.iter().position(|(_, dependencies, _)| {
                dependencies.iter().all(|id| available.contains_key(id))
            }) else {
                return Err(crate::LinkDataError(format!(
                    "missing or cyclic layout dependency for {}",
                    pending[0].0
                )));
            };
            let (id, _, record) = pending.remove(index);
            let record = record.read_link(target, foundation, identities, &available)?;
            if let Some(previous) = available.insert(id, record.clone()) {
                if previous.identity().physical_definition().provider() == foundation.producer() {
                    return Err(crate::LinkDataError(format!("duplicate layout {id}")));
                }
                if !previous.has_same_odr_definition(&record) {
                    return Err(crate::LinkDataError(format!(
                        "inconsistent layout definition {id}"
                    )));
                }
            }
            complete.push(record);
        }
        let table = CanonicalExactLayoutExportsV1::try_new(target, foundation, complete)
            .map_err(link_error)?;
        Ok(table)
    }

    pub fn validate_against(
        self,
        expected: &CanonicalExactLayoutExportsV1,
    ) -> Result<CanonicalExactLayoutExportsV1, ExactLayoutTableError> {
        if self.records.len() != expected.records().len() {
            return Err(ExactLayoutTableError::Count);
        }
        for (index, (record, expected)) in
            self.records.into_iter().zip(expected.records()).enumerate()
        {
            record
                .validate_against(expected)
                .map_err(|source| ExactLayoutTableError::Record { index, source })?;
        }
        Ok(expected.clone())
    }
}

fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    records: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}

impl WireEncode for CanonicalExactLayoutExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.records())
    }
}
impl WireEncode for DecodedCanonicalExactLayoutExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalExactLayoutExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactLayoutExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
