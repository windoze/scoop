use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(crate) fn imported_function_signature(
        &mut self,
        callable: hir::DefaultCallableDeclarationV1,
    ) -> Result<Option<hir::SemanticCallableSignature>, Diagnostic> {
        use hir::DefaultCallableDeclarationV1 as D;
        let identity = match callable {
            D::Function(function) => scoop_identity::CallableTemplateOrigin::Function(function),
            D::GenericFunction(function) => {
                scoop_identity::CallableTemplateOrigin::GenericFunction(function)
            }
            _ => return Ok(None),
        };
        let Some(dependencies) = &self.dependencies else {
            return Ok(None);
        };
        let declaration = dependencies
            .callable_declaration(identity)
            .map_err(|error| Diagnostic::at(Span::new(0, 0), error.to_string()))?;
        let interface = declaration.interface();
        let origin = imported_location(declaration.definition_origin().origin())?;
        let owner_parameters = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => self
                .imported_callable_owner_parameters(owner)
                .map_err(|error| {
                    Diagnostic::at(origin.span, error.diagnostic("queried callable owner"))
                })?,
            hir::PublicDeclarationOwnerV1::TopLevel | hir::PublicDeclarationOwnerV1::Extension => {
                Vec::new()
            }
        };
        let type_parameters =
            self.query_imported_method_binders(&declaration, &owner_parameters, origin.span)?;
        let bindings = self.query_signature_bindings(owner_parameters.len(), &type_parameters);
        let receiver = interface
            .receiver()
            .map(|ty| self.imported_signature_type_with_bindings(ty, &bindings))
            .transpose()
            .map_err(|error| {
                Diagnostic::at(origin.span, error.diagnostic("queried callable receiver"))
            })?;
        let context_parameters = self
            .imported_context_parameters(interface.context_parameters(), &bindings, origin.span)
            .map_err(|error| Diagnostic::at(origin.span, error))?;
        let substitutions = self.complete_parameter_substitution(type_parameters.iter(), &[]);
        let parameters = self.query_imported_parameters(&declaration, &bindings, &substitutions)?;
        let result = self
            .imported_signature_type_with_bindings(interface.result(), &bindings)
            .map_err(|error| {
                Diagnostic::at(origin.span, error.diagnostic("queried callable result"))
            })?;
        let effects = interface.effects();
        let release_callability = self
            .imported_release_callability(&effects, &bindings)
            .map_err(|error| Diagnostic::at(origin.span, error))?;
        Ok(Some(hir::SemanticFunctionSignature {
            function: hir::SemanticCallable::Imported(callable),
            name: declaration.name().into(),
            receiver,
            parameters,
            owner_parameter_count: owner_parameters.len(),
            type_parameters,
            context_parameters,
            result,
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: effects.callable_modifiers(),
            attributes: effects.function_attributes(),
            release_callability,
            origin,
        }))
    }
}
