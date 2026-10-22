//! Logical body projection for a frontend view before module publication.

use super::{entities::DefaultEntityProjector, envelope, generic, initialization};
use crate::*;

impl<'a> HirBodyProjectionInputs<'a> {
    pub fn project_struct_constructor(
        self,
        constructor: StructConstructorId,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<ExportConstructorInitializationV1, GenericTemplateProductionError> {
        initialization::struct_constructor(
            &DefaultEntityProjector::from_inputs(self, imported),
            constructor,
        )
    }

    pub fn project_class_constructor(
        self,
        constructor: ClassConstructorId,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<ExportConstructorInitializationV1, GenericTemplateProductionError> {
        initialization::class_constructor(
            &DefaultEntityProjector::from_inputs(self, imported),
            constructor,
        )
    }

    pub fn project_class_common_initialization(
        self,
        class: ClassId,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<Vec<ExportCommonInitializationStepV1>, GenericTemplateProductionError> {
        initialization::class_common(&DefaultEntityProjector::from_inputs(self, imported), class)
    }

    pub fn project_callable_body(
        self,
        function: FunctionId,
        origin: DefinitionOrigin,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<Option<ExportGenericCallableBodyV1>, GenericTemplateProductionError> {
        let entities = DefaultEntityProjector::from_inputs(self, imported);
        generic::projection::project_with_origin(&entities, function, origin)
    }

    pub fn project_parameter_default(
        self,
        owner: ExportParameterOwner,
        position: u32,
        source: ExportDefaultSourceId,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
        let root = match owner {
            ExportParameterOwner::Function(id) => LexicalDefinitionRoot::Function(id),
            ExportParameterOwner::StructConstructor(id) => {
                LexicalDefinitionRoot::StructConstructor(id)
            }
            ExportParameterOwner::ClassConstructor(id) => {
                LexicalDefinitionRoot::ClassConstructor(id)
            }
            ExportParameterOwner::VariantConstructor(id) => {
                LexicalDefinitionRoot::VariantConstructor(id)
            }
        };
        let entities = DefaultEntityProjector::from_inputs(self, imported);
        let scope = envelope::scope::provider_scope(&entities, root)?;
        let key = ExportDefaultTemplateKeyV1::new(scope.root.declaration(), position);
        envelope::project_template(&entities, &scope.binders, key, source)
    }
}
