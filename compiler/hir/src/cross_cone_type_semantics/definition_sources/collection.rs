use std::{collections::BTreeSet, convert::Infallible};

use super::*;
use crate::ExportDefinitionSourceV1;

impl TypeDefinitionSourceInputsV1<'_> {
    /// Uses the reader's occurrence walk; collection does not validate source ownership.
    pub(crate) fn collect_definition_sources(
        self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<ExportDefinitionSourceV1>, TypeDefinitionSourceClosureError<Infallible>> {
        let mut collector = Collector::default();
        declarations::visit(self, &mut collector, meter, path)?;
        defaults::visit(self, &mut collector, meter, path)?;
        meter.charge_collection_slots(collector.sources.len() as u64, path)?;
        Ok(collector.sources.into_iter().collect())
    }
}

#[derive(Default)]
struct Collector {
    sources: BTreeSet<ExportDefinitionSourceV1>,
    max_bytes: u64,
}

impl SourceVisitor<Infallible> for Collector {
    fn observe(
        &mut self,
        source: &ExportDefinitionSourceV1,
        _source_use: TypeDefinitionSourceUseV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<Infallible>> {
        meter.charge_nodes(1, path)?;
        meter.check_semantic_leaf(
            source.origin().source().logical_path().as_str().len() as u64,
            path,
        )?;
        let bytes = validation::source_bytes(source, path)?;
        self.max_bytes = self.max_bytes.max(bytes);
        let comparisons = 2 * (u64::from(self.sources.len().max(1).ilog2()) + 1);
        meter.charge_work(self.max_bytes.saturating_mul(comparisons), path)?;
        if !self.sources.contains(source) {
            meter.check_table_entries(self.sources.len() as u64 + 1, path)?;
            meter.charge_collection_slots(1, path)?;
            meter.charge_owned_bytes(bytes, path)?;
            self.sources.insert(source.clone());
        }
        Ok(())
    }
}
