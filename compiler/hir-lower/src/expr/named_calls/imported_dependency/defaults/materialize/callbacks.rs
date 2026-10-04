//! Callback sites keep their original registration and lexical arguments.

use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_callback_registration(
        &mut self,
        registration: scoop_identity::PersistentCallbackRegistrationId,
        closure: &hir::DefaultExpressionV1,
        origin: hir::DefinitionOrigin,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let (identity, definition_origin) = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.callback_registration(registration))
            .map(|(identity, origin)| (identity.clone(), origin.clone()))
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "dependency callback is missing its original registration".into(),
                )
            })?;
        let native = identity.key().source_signature();
        let parameters = native
            .parameters()
            .iter()
            .map(|parameter| self.materialize_imported_default_type(parameter, context))
            .collect::<Result<Vec<_>, _>>()?;
        let result = match native.result() {
            scoop_identity::SourceCAbiReturn::Void => self.unit,
            scoop_identity::SourceCAbiReturn::Value(result) => {
                self.materialize_imported_default_type(result, context)?
            }
        };
        let native = self.intern_function_type(false, parameters, result);
        let hir::Type::Function(native_function_type) = self.types[native] else {
            unreachable!("function interning produces a function type")
        };
        let closure = self.materialize_imported_default_expression(closure, context)?;
        let hir::Type::Function(managed_function_type) = self.types[closure.ty] else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "dependency callback closure has a non-function type".into(),
            ));
        };
        let context_index = identity.key().context_index().get();
        let mode = identity.key().mode();
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    definition: hir::ForeignCallbackDefinition::Imported {
                        identity: Box::new(identity),
                        definition_origin: Box::new(definition_origin),
                        origin,
                        arguments: context.lexical_arguments.clone(),
                    },
                    native_function_type,
                    managed_function_type,
                    context_index,
                    mode,
                    span: origin.span,
                });
        Ok(hir::ExprKind::ForeignCallbackRegister {
            registration,
            closure: Box::new(closure),
        })
    }
}
