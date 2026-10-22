//! Logical body projection for a frontend view before module publication.

use super::{entities::DefaultEntityProjector, envelope, generic};
use crate::*;

impl<'a> HirBodyProjectionInputs<'a> {
    pub fn project_callable_body(
        self,
        function: FunctionId,
        origin: DefinitionOrigin,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<Option<ExportGenericCallableBodyV1>, GenericTemplateProductionError> {
        let entities = DefaultEntityProjector::from_inputs(self, imported);
        generic::projection::project_with_origin(&entities, function, origin)
    }

    pub fn project_default_template(
        self,
        source: ExportDefaultSourceId,
        key: ExportDefaultTemplateKeyV1,
        binders: &[HirSignatureBinder],
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
        let entities = DefaultEntityProjector::from_inputs(self, imported);
        envelope::project_template(&entities, binders, key, source)
    }
}
