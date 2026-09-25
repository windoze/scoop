use super::*;
use crate::{DefaultSourceOriginSiteV1, ExportDefinitionSourceV1};
use scoop_wire::WireError;

impl CanonicalDefaultSourceTemplatesV1 {
    /// Preserves every occurrence, including repeated origins and shared providers.
    pub fn visit_definition_sources<V, E>(&self, visitor: &mut V, path: &WirePath) -> Result<(), E>
    where
        V: FnMut(
            &DefaultSourceTemplateV1,
            &ExportDefinitionSourceV1,
            DefaultSourceOriginSiteV1<'_>,

            &WirePath,
        ) -> Result<(), E>,
        E: From<WireError>,
    {
        for (index, template) in self.records.iter().enumerate() {
            template.visit_definition_sources(
                &mut |source, site, path| visitor(template, source, site, path),
                &path.clone().index(index as u64),
            )?;
        }
        Ok(())
    }
}
