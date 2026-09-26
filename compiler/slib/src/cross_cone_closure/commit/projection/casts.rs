//! Implicit cast failures select an ordinary dependency constructor.

use scoop_hir::{ImportedCoreProtocolCallableDefinition, concrete};
use scoop_identity::StrongCallableDefinitionOwner;
use scoop_mir::{MirCallableLoweringRoleV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_cast_constructor(
        &self,
        module: &concrete::Module,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let concrete::ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
            return Ok(());
        };
        let mut required = false;
        module
            .visit_executable_expressions(|occurrence| {
                required |= matches!(
                    occurrence.expression.kind,
                    concrete::ExprKind::Cast {
                        optional: false,
                        ..
                    }
                );
                Ok::<_, std::convert::Infallible>(())
            })
            .map_err(|error| match error {
                concrete::ExecutableExpressionVisitError::Structure(error) => {
                    Error::Expressions(error)
                }
                concrete::ExecutableExpressionVisitError::Visitor(never) => match never {},
            })?;
        if !required {
            return Ok(());
        }
        let constructor = protocols.exceptions().class_cast_exception_constructor();
        let provider = constructor.provider();
        let target = match constructor.definition() {
            ImportedCoreProtocolCallableDefinition::Constructor(id) => {
                StrongCallableDefinitionOwner::Constructor(id.persistent())
            }
            ImportedCoreProtocolCallableDefinition::GeneratedCallable(id) => {
                StrongCallableDefinitionOwner::GeneratedCallable(id.persistent())
            }
            definition => return Err(Error::RuntimeConstructorKind(definition)),
        };
        if selected
            .iter()
            .any(|callable| callable.provider() == provider && callable.implementation() == target)
        {
            return Ok(());
        }
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider { provider })?;
        let definition = artifact
            .production()
            .layout()
            .mir_type_bridge()
            .exports()
            .callables()
            .get(target)
            .ok_or(Error::MissingExport { provider, target })?;
        if !matches!(
            definition.lowering_role(),
            MirCallableLoweringRoleV1::ClassInitializer { .. }
        ) || !definition
            .semantic_signature()
            .exact()
            .parameters()
            .is_empty()
        {
            return Err(Error::SignatureMismatch { provider, target });
        }
        selected.push(SelectedExternalMirCallable::from_lowered(
            provider,
            definition.clone(),
        ));
        Ok(())
    }
}
