//! Temporary canonical record bytes used once by artifact content hashing.

use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

pub(crate) struct RuntimeImageRecords {
    prefix: Vec<u8>,
    tables: [Vec<Vec<u8>>; 6],
}
impl RuntimeImageRecords {
    pub(crate) fn new(prefix: Vec<u8>) -> Self {
        Self {
            prefix,
            tables: std::array::from_fn(|_| Vec::new()),
        }
    }
    pub(crate) fn push(&mut self, table: usize, bytes: Vec<u8>) {
        self.tables[table].push(bytes);
    }
    pub(crate) fn fingerprint(&self) -> Result<crate::RuntimeImageFingerprint, HashError> {
        domain_separated_runtime_hash("scoop-runtime-image-v2", self)
            .map(|digest| crate::RuntimeImageFingerprint::from_array(*digest.as_array()))
    }
}
impl RuntimeEncode for RuntimeImageRecords {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(&self.prefix)?;
        for table in &self.tables {
            encoder.sequence_length(table.len())?;
            for record in table {
                encoder.fixed(record)?;
            }
        }
        Ok(())
    }
}
