use super::*;
use crate::{DefaultSourceOriginSiteV1, ExportDefinitionSourceV1};
use scoop_wire::WireError;

impl CanonicalDefaultSourceTemplatesV1 {
    /// Preserves every occurrence, including repeated origins and shared providers.
    pub fn visit_definition_sources_metered<V, E>(
        &self,
        visitor: &mut V,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(
            &DefaultSourceTemplateV1,
            &ExportDefinitionSourceV1,
            DefaultSourceOriginSiteV1<'_>,
            &mut BudgetMeter,
            &WirePath,
        ) -> Result<(), E>,
        E: From<WireError>,
    {
        meter.check_semantic_depth(1, path)?;
        meter.check_table_entries(self.records.len() as u64, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        for (index, template) in self.records.iter().enumerate() {
            template.visit_definition_sources_metered(
                &mut |source, site, meter, path| visitor(template, source, site, meter, path),
                meter,
                &path.clone().index(index as u64),
            )?;
        }
        Ok(())
    }
}
