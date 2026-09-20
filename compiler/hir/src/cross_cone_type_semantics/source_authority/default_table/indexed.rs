use super::*;
use crate::IndexedDefaultSourceTemplateV1;
use scoop_wire::{Encoder, WireEncode};
impl CanonicalDefaultSourceTemplatesV1 {
    pub fn index_locals(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<IndexedCanonicalDefaultSourceTemplatesV1<'_>, DefaultSourceTemplateTableIndexError>
    {
        use DefaultSourceTemplateTableIndexError as Error;
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter
            .charge_work((self.records.len() as u64).saturating_add(1), &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots(self.records.len() as u64, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_owned_bytes(
                (self.records.len() as u64).saturating_mul(std::mem::size_of::<
                    IndexedDefaultSourceTemplateV1<'_>,
                >() as u64),
                &path,
            )
            .map_err(Error::Resource)?;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.iter().enumerate() {
            records.push(
                record
                    .index_locals(meter)
                    .map_err(|error| Error::Template { index, error })?,
            );
        }
        Ok(IndexedCanonicalDefaultSourceTemplatesV1 { records })
    }
}
#[derive(Debug)]
pub struct IndexedCanonicalDefaultSourceTemplatesV1<'a> {
    records: Vec<IndexedDefaultSourceTemplateV1<'a>>,
}
impl WireEncode for IndexedCanonicalDefaultSourceTemplatesV1<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(e)?;
        }
        Ok(())
    }
}
