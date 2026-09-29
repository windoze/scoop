//! Targets shared by public envelopes and independent source occurrences.
use super::DefaultEntityProjector;
use crate::{ExportDefaultCallableTarget, ExportDefaultCallableTargetV1, HirSignatureBinder};
impl DefaultEntityProjector<'_> {
    pub(in crate::production::default_templates) fn reference_callable(
        &self,
        target: ExportDefaultCallableTarget,
        binders: &[HirSignatureBinder],
    ) -> Result<ExportDefaultCallableTargetV1, super::super::DefaultEntityProjectionError> {
        let entities = self;
        let export = entities.export();
        Ok(match target {
            ExportDefaultCallableTarget::Callable(callable) => {
                ExportDefaultCallableTargetV1::Callable(entities.callable(callable, binders)?)
            }

            ExportDefaultCallableTarget::ImportedDependency(callable) => {
                ExportDefaultCallableTargetV1::Callable(
                    entities.imported_dependency_callable(callable)?,
                )
            }
            ExportDefaultCallableTarget::ImportedGeneric(application) => {
                ExportDefaultCallableTargetV1::Callable(
                    self.imported_generic_callable(application, binders)?,
                )
            }
            ExportDefaultCallableTarget::Bound(bound) => {
                ExportDefaultCallableTargetV1::Bound(entities.bound_callable(bound, binders)?)
            }
            ExportDefaultCallableTarget::ImportedBound(bound) => {
                ExportDefaultCallableTargetV1::Bound(self.imported_bound_callable(&bound, binders)?)
            }
            ExportDefaultCallableTarget::DerivedEquality(id) => {
                let application =
                    super::super::arena_get(&export.derived_equality_applications, id).ok_or(
                        super::super::DefaultEntityProjectionError::Unknown {
                            kind: "derived equality application",
                            index: super::super::raw_index(id),
                        },
                    )?;
                ExportDefaultCallableTargetV1::DerivedEquality {
                    owner_type: entities.type_key(application.owner_ty, binders)?,
                }
            }
            ExportDefaultCallableTarget::LocalFunction(id) => {
                let function = super::super::arena_get(&export.local_functions, id).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "local function",
                        index: super::super::raw_index(id),
                    },
                )?;
                ExportDefaultCallableTargetV1::LocalFunction {
                    declaration: entities.source_callable_declaration(function.function)?,
                }
            }
            ExportDefaultCallableTarget::Lambda(id) => {
                let lambda = super::super::arena_get(&export.lambdas, id).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "lambda",
                        index: super::super::raw_index(id),
                    },
                )?;
                ExportDefaultCallableTargetV1::Lambda {
                    body: entities.generated_function_id(lambda.function)?,
                }
            }
            ExportDefaultCallableTarget::AnonymousFunction(id) => {
                let function = super::super::arena_get(&export.anonymous_functions, id).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "anonymous function",
                        index: super::super::raw_index(id),
                    },
                )?;
                ExportDefaultCallableTargetV1::AnonymousFunction {
                    body: entities.generated_function_id(function.function)?,
                }
            }
            ExportDefaultCallableTarget::CallableReference(id) => {
                let reference = super::super::arena_get(&export.callable_references, id).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "callable reference",
                        index: super::super::raw_index(id),
                    },
                )?;
                ExportDefaultCallableTargetV1::CallableReference {
                    invoke: entities.callable_reference_invoke(
                        reference.definition_root,
                        &reference.definition_path,
                    )?,
                }
            }
            ExportDefaultCallableTarget::FunctionAddress(function) => {
                ExportDefaultCallableTargetV1::FunctionAddress {
                    declaration: entities.callable_declaration(function)?,
                }
            }
        })
    }

    pub(in crate::production::default_templates) fn imported_generic_callable(
        &self,
        application: crate::ImportedGenericCallableApplicationId,
        binders: &[HirSignatureBinder],
    ) -> Result<crate::DefaultCallableRefV1, super::super::DefaultEntityProjectionError> {
        let export = self.export();
        let application = &export.imported_generic_applications[application];
        let declaration = export.imported_generic_templates[application.template]
            .declaration
            .body_owner();
        let (owner, arguments) = match &application.arguments {
            crate::ImportedCallableArguments::Function(arguments) => (
                scoop_identity::OptionalSignatureType::Absent,
                arguments.to_vec(),
            ),
            crate::ImportedCallableArguments::Method {
                owner,
                method_arguments,
            } => (
                scoop_identity::OptionalSignatureType::Present(Box::new(
                    self.type_key(*owner, binders)?,
                )),
                method_arguments.clone(),
            ),
        };
        let arguments = arguments
            .iter()
            .map(|ty| self.type_key(*ty, binders))
            .collect::<Result<Vec<_>, _>>()?;
        crate::DefaultCallableRefV1::try_new(declaration, owner, arguments)
            .map_err(super::super::DefaultEntityProjectionError::Callable)
    }
}
