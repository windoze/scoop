use super::*;

pub(crate) fn imported_location(
    origin: &scoop_identity::DefinitionOrigin,
) -> Result<hir::SemanticSourceLocation, Diagnostic> {
    let span = origin.span();
    let invalid = |_| {
        let mut diagnostic = Diagnostic::at(
            Span::new(0, 0),
            "queried dependency source range exceeds the supported byte range",
        );
        diagnostic.source = Some(Box::new(origin.source().clone()));
        diagnostic
    };
    Ok(hir::SemanticSourceLocation {
        source: origin.source().clone(),
        span: Span::new(
            u32::try_from(span.start_byte()).map_err(invalid)?,
            u32::try_from(span.end_byte()).map_err(invalid)?,
        ),
    })
}

impl Lowerer {
    pub(crate) fn in_context<T>(
        &mut self,
        context: hir::SemanticContext,
        operation: impl FnOnce(&mut Lowerer) -> T,
    ) -> T {
        let (file, owner, parameters) = match context {
            hir::SemanticContext::Function(function) => (
                self.function_files[&function],
                self.function_owner.get(&function).copied(),
                self.signatures[&function].type_params.clone(),
            ),
            hir::SemanticContext::Nominal(owner) => {
                let file = match owner {
                    hir::NominalOwner::Class(id) => self.class_files[&id],
                    hir::NominalOwner::Struct(id) => self.struct_files[&id],
                    hir::NominalOwner::Enum(id) => self.enum_files[&id],
                    hir::NominalOwner::Interface(id) => self.interface_files[&id],
                    hir::NominalOwner::Object(id) => self.object_files[&id],
                };
                let owner = Owner::from_nominal_owner(owner);
                (file, Some(owner), self.owner_type_params(owner))
            }
        };
        let previous_file = std::mem::replace(&mut self.current_file, file);
        let previous_query = std::mem::replace(&mut self.in_semantic_query, true);
        let previous_owner = std::mem::replace(&mut self.current_owner, owner);
        let previous_parameters = std::mem::replace(&mut self.type_params_in_scope, parameters);
        let previous_scopes = owner
            .and_then(|owner| self.local_nominal_scopes(owner))
            .map(|scopes| std::mem::replace(&mut self.scopes, scopes));
        let previous_functions = owner
            .and_then(|owner| self.local_nominal_functions(owner))
            .map(|functions| std::mem::replace(&mut self.local_function_scopes, functions));
        let value = operation(self);
        if let Some(scopes) = previous_scopes {
            self.scopes = scopes;
        }
        if let Some(functions) = previous_functions {
            self.local_function_scopes = functions;
        }
        self.current_file = previous_file;
        self.in_semantic_query = previous_query;
        self.current_owner = previous_owner;
        self.type_params_in_scope = previous_parameters;
        value
    }

    pub(crate) fn signature_binders(
        &self,
        context: hir::SemanticTypeContext<'_>,
    ) -> Vec<hir::HirSignatureBinder> {
        let context = match context {
            hir::SemanticTypeContext::Lexical(context) => context,
            hir::SemanticTypeContext::Declaration(binders) => return binders.signature_binders(),
        };
        let (parameters, owner_count) = match context {
            hir::SemanticContext::Function(function) => {
                let signature = &self.signatures[&function];
                (
                    signature.type_params.clone(),
                    signature.owner_type_param_count,
                )
            }
            hir::SemanticContext::Nominal(owner) => {
                let parameters = self.owner_type_params(Owner::from_nominal_owner(owner));
                let count = parameters.len();
                (parameters, count)
            }
        };
        let (owner, callable) = parameters.split_at(owner_count);
        hir::SemanticDeclarationBinders {
            owner: owner.to_vec(),
            callable: callable.to_vec(),
        }
        .signature_binders()
    }

    pub(crate) fn in_type_context<T>(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        operation: impl FnOnce(&mut Lowerer) -> T,
    ) -> T {
        match context {
            hir::SemanticTypeContext::Lexical(context) => self.in_context(context, operation),
            hir::SemanticTypeContext::Declaration(binders) => {
                let parameters = binders.parameters().cloned().collect();
                let previous = std::mem::replace(&mut self.type_params_in_scope, parameters);
                let previous_query = std::mem::replace(&mut self.in_semantic_query, true);
                let value = operation(self);
                self.in_semantic_query = previous_query;
                self.type_params_in_scope = previous;
                value
            }
        }
    }

    pub(crate) fn type_query(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        span: Span,
        operation: impl FnOnce(&mut Lowerer) -> Option<hir::TypeId>,
    ) -> Result<hir::TypeId, Vec<Diagnostic>> {
        self.in_type_context(context, |lowerer| {
            let before = lowerer.diagnostics.len();
            let result =
                operation(lowerer).filter(|ty| lowerer.check_query_type_representation(*ty, span));
            let mut diagnostics = lowerer.diagnostics.split_off(before);
            let sources = lowerer.diagnostic_source_identities();
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            match result {
                Some(ty) if diagnostics.is_empty() => Ok(ty),
                _ => Err(diagnostics),
            }
        })
    }
}
