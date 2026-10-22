use super::*;
use crate::call_resolution::applicability::DeclarationTypeArguments;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn editing_imported_call_candidate(
        &mut self,
        call: &ast::RecoveredCall,
        declaration: &hir::ImportedCallableDeclaration,
        receiver: Option<hir::TypeId>,
    ) -> Candidate {
        let interface = declaration.interface();
        if !self.imported_callable_is_accessible(interface, receiver) {
            return Ok(None);
        }
        let (owner_parameters, owner_arguments) = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => {
                let has_parameters = !self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nominal_declaration(owner))
                    .ok_or(U::SignatureFailure)?
                    .interface
                    .type_parameters()
                    .binders()
                    .is_empty();
                let parameters = if has_parameters {
                    self.nominal_declaration_parameters(owner)
                        .map_err(|_| U::SignatureFailure)?
                } else {
                    Vec::new()
                };
                let arguments = if let Some(receiver) = receiver {
                    let Some(owner_type) = self.imported_member_owner_type(receiver, owner) else {
                        return Ok(None);
                    };
                    if let hir::Type::Ptr(pointee) = self.types[owner_type] {
                        vec![pointee]
                    } else {
                        self.nominal_application(owner_type)
                            .map_or_else(Vec::new, |application| application.arguments)
                    }
                } else if parameters.is_empty() {
                    Vec::new()
                } else {
                    return Err(U::PendingInference);
                };
                (parameters, arguments)
            }
            hir::PublicDeclarationOwnerV1::TopLevel | hir::PublicDeclarationOwnerV1::Extension => {
                (Vec::new(), Vec::new())
            }
        };
        let signature =
            self.editing_imported_signature(declaration, owner_parameters, call.span)?;
        let parameters = signature
            .owner_parameters
            .iter()
            .chain(&signature.callable_parameters)
            .cloned()
            .collect::<Vec<_>>();
        let bindings = self.query_signature_bindings(signature.owner_parameters.len(), &parameters);
        let bound_receiver = if interface.owner() == hir::PublicDeclarationOwnerV1::Extension {
            let Some(actual) = receiver else {
                return Ok(None);
            };
            let declared = self
                .imported_signature_type_with_bindings(
                    interface.receiver().ok_or(U::SignatureFailure)?,
                    &bindings,
                )
                .map_err(|_| U::SignatureFailure)?;
            Some((declared, actual))
        } else {
            None
        };
        let return_type = signature.return_type;
        self.editing_signature_candidate(
            call,
            inference::CandidateInput {
                signature,
                type_arguments: DeclarationTypeArguments::Callable {
                    owner_arguments: &owner_arguments,
                },
                receiver: bound_receiver,
                argument_mode: ArgumentMode::Mixed,
                result: V::Known(return_type),
                named_parameters: true,
                explicit_arguments: None,
            },
        )
    }

    pub(crate) fn editing_imported_signature(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        owner_parameters: Vec<hir::TypeParamDecl>,
        span: ast::Span,
    ) -> Result<DeclarationSignature<hir::ExportDefaultTemplateKeyV1>, U> {
        let interface = declaration.interface();
        let parameters = self
            .query_imported_method_binders(declaration, &owner_parameters, span)
            .map_err(|_| U::SignatureFailure)?;
        let bindings = self.query_signature_bindings(owner_parameters.len(), &parameters);
        let types = interface
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                self.imported_signature_type_with_bindings(parameter.value_type(), &bindings)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| U::SignatureFailure)?;
        let value_parameters = self.imported_parameter_views(declaration, types);
        let return_type = self
            .imported_signature_type_with_bindings(interface.result(), &bindings)
            .map_err(|_| U::SignatureFailure)?;
        Ok(DeclarationSignature {
            callable_parameters: parameters[owner_parameters.len()..].to_vec(),
            owner_parameters,
            value_parameters,
            return_type,
        })
    }
}
