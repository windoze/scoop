use std::{collections::BTreeSet, convert::Infallible};

use super::*;
use crate::ExportDefinitionSourceV1;

impl TypeDefinitionSourceInputsV1<'_> {
    /// Uses the reader's occurrence walk; collection does not validate source ownership.
    pub(crate) fn collect_definition_sources(
        self,

        path: &WirePath,
    ) -> Result<Vec<ExportDefinitionSourceV1>, TypeDefinitionSourceClosureError<Infallible>> {
        let mut collector = Collector::default();
        declarations::visit(self, &mut collector, path)?;
        defaults::visit(self, &mut collector, path)?;

        Ok(collector.sources.into_iter().collect())
    }
}

#[derive(Default)]
struct Collector {
    sources: BTreeSet<ExportDefinitionSourceV1>,
}

impl SourceVisitor<Infallible> for Collector {
    fn observe(
        &mut self,
        source: &ExportDefinitionSourceV1,
        _source_use: TypeDefinitionSourceUseV1<'_>,

        _path: &WirePath,
    ) -> Result<(), TypeDefinitionSourceClosureError<Infallible>> {
        if !self.sources.contains(source) {
            self.sources.insert(source.clone());
        }
        Ok(())
    }
}
