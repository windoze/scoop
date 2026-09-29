//! Default-body data flow using the already validated declaration closure.

use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultDataFlowError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_local_data_flow(
        &mut self,
    ) -> Result<(), CrossConeHirDefaultDataFlowError> {
        let templates = self.current_interface.default_templates();
        if templates.records().is_empty() {
            return Ok(());
        }
        let path = WirePath::root().field(7);
        for (index, template) in templates.records().iter().enumerate() {
            template
                .validate_local_data_flow_semantics(&path)
                .map_err(|source| CrossConeHirDefaultDataFlowError::Template {
                    index,
                    key: template.key(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }
}
