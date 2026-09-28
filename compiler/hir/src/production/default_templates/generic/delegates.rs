use super::*;
use crate::{
    CanonicalExportGenericDelegatesV1, ExportGenericDelegateTemplateV1, GenericDelegateTemplateId,
};

impl GenericBodyProducer<'_> {
    pub(in crate::production) fn delegate_template(
        &self,
        template: GenericDelegateTemplateId,
    ) -> Result<ExportGenericDelegateTemplateV1, GenericTemplateProductionError> {
        let export = self.entities.export();
        let template = &export.generic_delegate_templates[template];
        let unit = &export.initialization_units[template.initialization];
        let property = export.property_identities[template.property]
            .extension_id()
            .expect("a generic delegate retains its extension declaration");
        let initializer = projection::project(&self.entities, unit.initializer)?
            .expect("a source delegate initializer has a checked executable body");
        let binders = crate::production::signatures::HirInterfaceSignatureProjector::new(export)
            .function_binders(&export.functions[unit.initializer])
            .map_err(GenericTemplateProductionError::Signature)?;
        let effective_type = self
            .entities
            .type_key(template.ty, &binders)
            .map_err(GenericTemplateProductionError::Entity)?;
        ExportGenericDelegateTemplateV1::try_new(
            property,
            effective_type,
            initializer,
            unit.display_name.clone(),
        )
        .map_err(GenericTemplateProductionError::Delegate)
    }
}

impl CanonicalExportGenericDelegatesV1 {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, GenericTemplateProductionError> {
        SharedSourceRoots::with_callable_bodies(export, None, &[])
            .map(|(_, _, _, delegates)| delegates)
    }

    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
    ) -> Result<Self, GenericTemplateProductionError> {
        Ok(output.output().export.shared_source().delegates.clone())
    }
}
