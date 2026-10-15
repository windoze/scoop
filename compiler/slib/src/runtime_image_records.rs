//! Runtime record bytes produced once and selected by the final program link.

use std::collections::BTreeSet;

use scoop_identity::ObjectDefinitionPlanId;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

mod wire;
pub(crate) use wire::DecodedRuntimeImageRecords;

const DOMAIN: &str = "scoop-runtime-image-v2";

#[derive(Clone, Debug, Eq, PartialEq)]
struct Record<I> {
    definition: I,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Records<I> {
    prefix: Vec<u8>,
    tables: [Vec<Record<I>>; 6],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeImageRecords(Records<ObjectDefinitionPlanId>);

impl RuntimeImageRecords {
    pub(crate) fn new(prefix: Vec<u8>) -> Self {
        Self(Records {
            prefix,
            tables: std::array::from_fn(|_| Vec::new()),
        })
    }

    pub(crate) fn push(
        &mut self,
        table: usize,
        definition: ObjectDefinitionPlanId,
        bytes: Vec<u8>,
    ) {
        self.0.tables[table].push(Record { definition, bytes });
    }

    pub(crate) fn fingerprint(&self) -> Result<crate::RuntimeImageFingerprint, HashError> {
        self.hash(None)
    }

    pub fn fingerprint_for_definitions(
        &self,
        selected: &BTreeSet<ObjectDefinitionPlanId>,
    ) -> Result<crate::RuntimeImageFingerprint, HashError> {
        self.hash(Some(selected))
    }

    fn hash(
        &self,
        selected: Option<&BTreeSet<ObjectDefinitionPlanId>>,
    ) -> Result<crate::RuntimeImageFingerprint, HashError> {
        domain_separated_runtime_hash(
            DOMAIN,
            &Selection {
                records: self,
                selected,
            },
        )
        .map(|digest| crate::RuntimeImageFingerprint::from_array(*digest.as_array()))
    }
}

struct Selection<'a> {
    records: &'a RuntimeImageRecords,
    selected: Option<&'a BTreeSet<ObjectDefinitionPlanId>>,
}

impl RuntimeEncode for Selection<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(&self.records.0.prefix)?;
        for table in &self.records.0.tables {
            let rows = table.iter().filter(|record| {
                self.selected
                    .is_none_or(|selected| selected.contains(&record.definition))
            });
            encoder.sequence_length(rows.clone().count())?;
            for record in rows {
                encoder.fixed(&record.bytes)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
