use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

use super::*;

#[derive(Debug)]
pub struct DecodedCanonicalExactLayoutExportsV1 {
    records: Vec<DecodedExactLayoutExportV1>,
}

impl DecodedCanonicalExactLayoutExportsV1 {
    pub(crate) fn read_link_values(
        self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<
        std::collections::BTreeMap<
            scoop_identity::PersistentExactTypeId,
            crate::LinkValueRepresentation,
        >,
        crate::LinkDataError,
    > {
        self.records
            .into_iter()
            .filter_map(|record| record.link_value(identities).transpose())
            .collect()
    }

    pub fn read(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<CanonicalExactLayoutExportsV1, crate::LinkDataError> {
        use crate::link_data::link_error;
        let mut prepared = Vec::with_capacity(self.records.len());
        for raw in self.records {
            let record = raw.prepare(target, foundation, identities)?;
            if prepared.last().is_some_and(
                |previous: &crate::exact_layout::wire::read::saved::PreparedLayout| {
                    previous.layout() >= record.layout()
                },
            ) {
                return Err(link_error("layout table is not in canonical order"));
            }
            prepared.push(record);
        }
        let records = prepared
            .into_iter()
            .map(|record| record.finish(foundation, identities))
            .collect::<Result<_, _>>()?;
        Ok(CanonicalExactLayoutExportsV1(Arc::new(LayoutTable {
            provider: foundation.producer(),
            target,
            records,
        })))
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
