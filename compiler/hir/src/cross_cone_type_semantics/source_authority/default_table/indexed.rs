use super::*;
use crate::IndexedDefaultSourceTemplateV1;
use scoop_wire::{Encoder, WireEncode};
impl CanonicalDefaultSourceTemplatesV1 {
    pub fn index_locals(
        &self,
    ) -> Result<IndexedCanonicalDefaultSourceTemplatesV1<'_>, DefaultSourceTemplateTableIndexError>
    {
        use DefaultSourceTemplateTableIndexError as Error;
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.iter().enumerate() {
            records.push(
                record
                    .index_locals()
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
