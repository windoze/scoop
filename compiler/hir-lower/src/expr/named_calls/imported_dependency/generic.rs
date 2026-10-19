//! Generic dependency candidates use the same inference session and solver as
//! current declarations. Their successful result retains a provider template.

use super::*;

mod diagnostics;
mod signature;
use crate::call_resolution::applicability::DeclarationTypeArguments;
use crate::call_resolution::contextual::{ArgumentExpression, ArgumentInferenceFailureKind};
use crate::call_resolution::probe::{CallInferenceInput, InferredCall};
pub(in crate::expr) use signature::{ImportedGenericTarget, LoadedCallableSignature};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_imported_generic(
        mut self: Box<Self>,
        candidate: ImportedCallableCandidate,
        declaration: super::signature::GenericCallDeclaration,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<hir::TypeId>,
        receiver: ImportedCallReceiver,
        argument_map: ImportedArgumentMap,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let super::signature::GenericCallDeclaration {
            template,
            signature,
            bindings: template_bindings,
        } = declaration;
        let mut explicit = match self.resolve_call_type_args(call.type_args) {
            Some(arguments) => arguments,
            None => return Err(self),
        };
        if matches!(template, ImportedGenericTarget::Constructor(_))
            && call.alias.is_none()
            && let ImportedCallReceiver::Member { value, .. } = &receiver
        {
            let outer = self
                .nominal_application(value.ty())
                .expect("an inner constructor has its outer application")
                .arguments;
            let own_count = signature.signature.owner_parameters.len() - outer.len();
            if explicit.is_empty() {
                explicit.extend(
                    (0..own_count)
                        .map(|_| crate::expr::ResolvedCallTypeArgument::Infer { span: call.span }),
                );
            }
            explicit.splice(
                0..0,
                outer
                    .into_iter()
                    .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit {
                        ty,
                        span: call.span,
                    }),
            );
        }
        let alias_signature = if let Some(alias) = call.alias {
            match self.alias_constructor_signature(
                alias,
                &signature.signature,
                &explicit,
                call.span,
            ) {
                Some(signature) => Some(signature),
                None => return Err(self),
            }
        } else {
            None
        };
        let inference_signature = alias_signature
            .as_ref()
            .map_or(&signature.signature, |alias| &alias.signature);
        let inference_explicit = alias_signature
            .as_ref()
            .map_or(explicit.as_slice(), |alias| alias.explicit.as_slice());
        let forced_hint = if candidate.callback_intrinsic()
            == Some(hir::IntrinsicFunctionKind::ForeignCallbackRegister)
        {
            use crate::call_resolution::arguments::ResolvedParameterInput;
            let ResolvedParameterInput::Explicit(context) =
                argument_map.mapping().parameters[1].input
            else {
                unreachable!("the context index is a required argument");
            };
            let ResolvedParameterInput::Explicit(callback) =
                argument_map.mapping().parameters[0].input
            else {
                unreachable!("the callback closure is a required argument");
            };
            let Some(context) = call.arguments.source(context.index()) else {
                self.error(
                    call.span,
                    "foreign callback `contextIndex` must be a compile-time integer literal".into(),
                );
                return Err(self);
            };
            match self.foreign_callback_expected(&explicit, context, callback.index(), call.span) {
                Ok(hint) => Some(hint),
                Err(()) => return Err(self),
            }
        } else {
            None
        };
        let nominal = matches!(
            template,
            ImportedGenericTarget::Constructor(_) | ImportedGenericTarget::Variant(_)
        );
        let owner_arguments = if nominal || signature.signature.owner_parameters.is_empty() {
            Vec::new()
        } else {
            let ImportedCallReceiver::Member { value, .. } = &receiver else {
                unreachable!("nominal member inference has its exact receiver");
            };
            let hir::PublicDeclarationOwnerV1::Nominal(owner) = candidate.interface().owner()
            else {
                unreachable!("owner parameters belong to a nominal member")
            };
            let owner = self
                .imported_member_owner_type(value.ty(), owner)
                .expect("nominal member inference retains its declared owner application");
            self.imported_owner_arguments(owner).to_vec()
        };
        let bound_receiver = match (signature.receiver, &receiver) {
            (Some(expected), ImportedCallReceiver::Member { value, .. }) => {
                let expected = alias_signature.as_ref().map_or(expected, |alias| {
                    self.instantiate_method_ty(expected, &alias.target_bindings)
                });
                Some((expected, value.ty()))
            }
            _ => None,
        };
        let expected_application = expected.and_then(|ty| self.nominal_application(ty));
        let type_arguments = if alias_signature.is_some() {
            DeclarationTypeArguments::Callable {
                owner_arguments: &[],
            }
        } else if nominal {
            let result = self
                .nominal_application(signature.signature.return_type)
                .expect("a nominal candidate retains its full result application");
            DeclarationTypeArguments::Nominal {
                template: result.template,
                expected_arguments: expected_application
                    .as_ref()
                    .filter(|application| application.template == result.template)
                    .map(|application| application.arguments.as_slice()),
            }
        } else {
            DeclarationTypeArguments::Callable {
                owner_arguments: &owner_arguments,
            }
        };
        let address_place =
            if candidate.pointer_intrinsic() == Some(hir::PointerIntrinsic::AddressOf) {
                let Some(source) = call.arguments.source(0) else {
                    self.error(
                        call.span,
                        "`addressOf` requires an addressable source argument".into(),
                    );
                    return Err(self);
                };
                match self.addressable_source_place(source) {
                    Some(place) => Some(place),
                    None => return Err(self),
                }
            } else {
                None
            };
        let expressions = call.arguments.expressions();
        let expressions = if let Some((place, ty)) = address_place.as_ref() {
            vec![ArgumentExpression::Addressable {
                place,
                ty: *ty,
                span: call.arguments.span(0),
            }]
        } else {
            expressions
        };
        let InferredCall {
            types: mut solution,
            mut bindings,
            values: source_args,
            sinks: argument_sinks,
            parameter_types,
            return_type: result_type,
            numeric_arguments,
        } = match self.infer_call_arguments(CallInferenceInput {
            signature: inference_signature,
            result: crate::declaration_inference::DeclarationResult::Known(
                inference_signature.return_type,
            ),
            argument_map: argument_map.mapping(),
            type_arguments,
            explicit_arguments: inference_explicit,
            bound_receiver,
            expressions: &expressions,
            expected_result: if nominal && alias_signature.is_none() {
                None
            } else {
                expected
            },
            forced_hint,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                if let ArgumentInferenceFailureKind::Constraint(failure) = failure.kind {
                    if alias_signature.is_some() {
                        let message = self.render_imported_constraint_failure(
                            &inference_signature.owner_parameters,
                            &inference_signature.callable_parameters,
                            &failure,
                        );
                        self.error(
                            call.span,
                            format!("typealias constructor `{}`: {message}", name.text),
                        );
                    } else {
                        self.imported_generic_inference_error(name, call, &signature, &failure);
                    }
                }
                return Err(self);
            }
        };
        if let Some(alias) = &alias_signature {
            bindings = alias
                .target_bindings
                .iter()
                .map(|(parameter, ty)| (*parameter, self.instantiate_method_ty(*ty, &bindings)))
                .collect();
            solution.owner = signature
                .signature
                .owner_parameters
                .iter()
                .map(|parameter| {
                    bindings
                        .iter()
                        .find(|(id, _)| *id == parameter.id)
                        .expect("constructor owner binding is complete")
                        .1
                })
                .collect();
            solution.callable = signature
                .signature
                .callable_parameters
                .iter()
                .map(|parameter| {
                    bindings
                        .iter()
                        .find(|(id, _)| *id == parameter.id)
                        .expect("constructor callable binding is complete")
                        .1
                })
                .collect();
        }
        let crate::declaration_inference::DeclarationResult::Known(result_type) = result_type
        else {
            unreachable!("imported signatures already have complete results")
        };
        let receiver = match receiver {
            ImportedCallReceiver::Member {
                value: ImportedMemberReceiver::Value(value),
                static_type,
            } => {
                let ty = self.instantiate_method_ty(
                    signature
                        .receiver
                        .expect("generic receiver has its declaration type"),
                    &bindings,
                );
                let value = if matches!(
                    candidate.interface().owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(_)
                ) && matches!(
                    self.types[value.ty],
                    hir::Type::Param(_) | hir::Type::Companion(_)
                ) && matches!(self.types[ty], hir::Type::Interface(_))
                {
                    value
                } else {
                    self.adapt_to(value, ty)
                };
                ImportedCallReceiver::Member {
                    value: ImportedMemberReceiver::Value(value),
                    static_type,
                }
            }
            receiver => receiver,
        };
        let default_bindings = template_bindings
            .iter()
            .map(|(key, ty)| (key.clone(), self.instantiate_method_ty(*ty, &bindings)))
            .collect();
        let default_plan = match self.prepare_imported_defaults_with_bindings(
            &candidate,
            &argument_map,
            &default_bindings,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.error(call.span, error.to_string());
                return Err(self);
            }
        };
        let implementation = if let Some(kind) = candidate.callback_intrinsic() {
            ImportedCallImplementation::Intrinsic {
                template,
                operation: ImportedIntrinsicCall::ForeignCallback {
                    kind,
                    native_type: solution.callable[0],
                },
            }
        } else if let Some(kind) = candidate.maybe_uninit_intrinsic()
            && (kind == hir::MaybeUninitIntrinsic::AssumeInit
                || matches!(&receiver, ImportedCallReceiver::Member { value: ImportedMemberReceiver::Value(value), .. } if matches!(value.kind, hir::ExprKind::SingletonValue(_))))
        {
            ImportedCallImplementation::Intrinsic {
                template,
                operation: ImportedIntrinsicCall::MaybeUninit(kind),
            }
        } else if let Some(kind) = candidate.atomic_intrinsic() {
            ImportedCallImplementation::Intrinsic {
                template,
                operation: ImportedIntrinsicCall::Atomic(kind),
            }
        } else if let Some(operation) = candidate.array_intrinsic() {
            ImportedCallImplementation::Intrinsic {
                template,
                operation,
            }
        } else if let Some(intrinsic) = candidate.pointer_intrinsic() {
            if intrinsic == hir::PointerIntrinsic::Cast {
                self.pointer_type_uses
                    .push((result_type, self.current_file, call.span));
            }
            let expression = match intrinsic {
                hir::PointerIntrinsic::AddressOf => {
                    let (place, ty) =
                        address_place.expect("address intrinsic retains its source place");
                    self.normalize_address_of(
                        place,
                        ty,
                        solution.callable[0],
                        call.arguments.span(0),
                        call.span,
                    )
                }
                hir::PointerIntrinsic::SizeOf | hir::PointerIntrinsic::AlignOf => {
                    self.normalize_layout_intrinsic(intrinsic, solution.callable[0], call.span)
                }
                _ => None,
            };
            let operation = match intrinsic {
                hir::PointerIntrinsic::AddressOf
                | hir::PointerIntrinsic::SizeOf
                | hir::PointerIntrinsic::AlignOf => match expression {
                    Some(expression) => ImportedIntrinsicCall::Expression(expression),
                    None => return Err(self),
                },
                _ => ImportedIntrinsicCall::PointerMember(intrinsic),
            };
            ImportedCallImplementation::Intrinsic {
                template,
                operation,
            }
        } else {
            let arguments = match &template {
                ImportedGenericTarget::Function(id)
                    if matches!(
                        self.imported_generic_templates[*id].declaration,
                        hir::ImportedCallableTemplateOrigin::Nominal { .. }
                    ) =>
                {
                    hir::ImportedCallableArguments::Method {
                        owner: self.instantiate_method_ty(
                            signature.receiver.expect("a nominal method has an owner"),
                            &bindings,
                        ),
                        method_arguments: solution.callable,
                    }
                }
                ImportedGenericTarget::Constructor(_) | ImportedGenericTarget::Variant(_) => {
                    hir::ImportedCallableArguments::Function(solution.owner)
                }
                _ => hir::ImportedCallableArguments::Function(solution.callable),
            };
            ImportedCallImplementation::Generic {
                template,
                arguments,
            }
        };
        Ok(ImportedDependencyCallProbe {
            alias: call.alias.cloned(),
            implementation,
            declaration_file: signature.origin.file as usize,
            declaration_span: signature.span,
            state: self,
            candidate,
            declared_receiver: signature.receiver,
            signature: signature.signature,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            numeric_arguments,
            call_span: call.span,
        })
    }
}
