use super::DefaultEntityProjector;
use crate::{DefaultClassConstructorIdV1, ExportParameterOwner};
use scoop_identity::CallableTemplateOrigin;

impl DefaultEntityProjector<'_> {
    pub(in crate::production::default_templates) fn parameter_owner(
        &self,
        owner: ExportParameterOwner,
    ) -> Result<CallableTemplateOrigin, super::super::DefaultEntityProjectionError> {
        Ok(match owner {
            ExportParameterOwner::Function(function) => {
                self.source_callable_declaration(function)?
            }
            ExportParameterOwner::StructConstructor(constructor) => {
                CallableTemplateOrigin::Constructor(self.struct_constructor_id(constructor)?)
            }
            ExportParameterOwner::ClassConstructor(constructor) => match self
                .class_constructor_id(constructor)?
            {
                DefaultClassConstructorIdV1::Source(id) => CallableTemplateOrigin::Constructor(id),
                DefaultClassConstructorIdV1::Generated(_) => {
                    return Err(
                        super::super::DefaultEntityProjectionError::MissingIdentity {
                            kind: "source class constructor",
                            index: super::super::raw_index(constructor),
                        },
                    );
                }
            },
            ExportParameterOwner::VariantConstructor(variant) => {
                CallableTemplateOrigin::VariantConstructor(self.variant_id(variant)?)
            }
        })
    }
}
