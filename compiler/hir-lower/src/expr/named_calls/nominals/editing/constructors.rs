use super::*;
use crate::call_resolution::candidates::{ValueParameter, ValueParameterCalling};

impl Lowerer {
    pub(super) fn editing_nominal_signature<D: Copy>(
        &mut self,
        call: &ast::RecoveredCall,
        signature: DeclarationSignature<D>,
        argument_mode: ArgumentMode,
        expected: Option<TypeId>,
        alias: Option<&crate::aliases::AliasTemplate>,
    ) -> Candidate {
        if let Some(alias) = alias {
            let explicit = self
                .resolve_call_type_args(&call.type_args)
                .ok_or(U::BodyFailure)?;
            let mapped = self
                .alias_constructor_signature(alias, &signature, &explicit, call.span)
                .ok_or(U::PendingInference)?;
            let result = V::Known(mapped.signature.return_type);
            return self.editing_signature_candidate(
                call,
                CandidateInput {
                    signature: mapped.signature,
                    type_arguments: DeclarationTypeArguments::Callable {
                        owner_arguments: &[],
                    },
                    receiver: None,
                    argument_mode,
                    result,
                    named_parameters: true,
                    explicit_arguments: Some(&mapped.explicit),
                },
            );
        }
        let template = self
            .nominal_application(signature.return_type)
            .map(|application| application.template)
            .or_else(|| self.imported_nominal_owner(signature.return_type))
            .or_else(|| {
                matches!(self.types[signature.return_type], Type::Ptr(_))
                    .then(|| {
                        self.ffi_ptr.map(|owner| {
                            self.nominal_identity(crate::Owner::Struct(owner))
                                .declaration_id()
                        })
                    })
                    .flatten()
            })
            .ok_or(U::SignatureFailure)?;
        let expected_arguments =
            self.named_nominal_expected_arguments(signature.return_type, expected);
        let result = V::Known(signature.return_type);
        self.editing_signature_candidate(
            call,
            CandidateInput {
                signature,
                type_arguments: DeclarationTypeArguments::Nominal {
                    template,
                    expected_arguments: expected_arguments.as_deref(),
                },
                receiver: None,
                argument_mode,
                result,
                named_parameters: true,
                explicit_arguments: None,
            },
        )
    }

    pub(super) fn editing_imported_constructor(
        &mut self,
        call: &ast::RecoveredCall,
        declaration: &hir::ImportedCallableDeclaration,
        expected: Option<TypeId>,
        alias: Option<&crate::aliases::AliasTemplate>,
    ) -> Candidate {
        if !self.imported_callable_is_accessible(declaration.interface(), None) {
            return Ok(None);
        }
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = declaration.interface().owner() else {
            return Err(U::SignatureFailure);
        };
        let parameters = self
            .nominal_declaration_parameters(owner)
            .map_err(|_| U::SignatureFailure)?;
        let Some(mode) =
            self.imported_call_argument_mode(declaration, header(call).callee, call.span)
        else {
            return Ok(None);
        };
        let signature = self.editing_imported_signature(declaration, parameters, call.span)?;
        self.editing_nominal_signature(call, signature, mode, expected, alias)
    }

    pub(super) fn editing_pointer_signature(
        &mut self,
        call: &ast::RecoveredCall,
        origin: crate::call_resolution::named::NamedIntrinsicStructOrigin,
        expected: Option<TypeId>,
        alias: Option<&crate::aliases::AliasTemplate>,
    ) -> Candidate {
        let parameters = origin
            .type_parameters(self, call.span)
            .map_err(|_| U::SignatureFailure)?;
        let pointee = self.intern_type(Type::Param(parameters[0].id));
        let return_type = self.intern_type(Type::Ptr(pointee));
        let signature = DeclarationSignature::<()> {
            owner_parameters: parameters,
            callable_parameters: Vec::new(),
            value_parameters: vec![ValueParameter {
                name: "raw".to_owned(),
                ty: self.integer_type(hir::IntegerKind::UNSIGNED_64),
                calling: ValueParameterCalling::Required,
            }],
            return_type,
        };
        self.editing_nominal_signature(call, signature, ArgumentMode::Mixed, expected, alias)
    }
}
