//! Imported declarations remain dependency uses or template applications.

use super::*;
use crate::call_resolution::applicability::CallableReferenceApplicabilityInput;
use crate::call_resolution::candidates::CallableEffects;
use crate::call_resolution::diagnostics::render_type_parameters;
use crate::call_resolution::specificity::DeclarationForwardingView;
use crate::expr::named_calls::imported_dependency::{
    DependencySignature, ImportedCallableCandidate, ImportedGenericTarget,
};
use hir::ImportedCallableSource;

enum ImportedReferenceImplementation {
    Dependency,
    Template(hir::ImportedGenericCallableTemplateId),
}

pub(super) struct ImportedReferenceDeclaration {
    candidate: ImportedCallableCandidate,
    implementation: ImportedReferenceImplementation,
    signature: DependencySignature,
    receiver: Option<TypeId>,
}

impl ImportedReferenceDeclaration {
    fn resolve(state: &mut Lowerer, candidate: ImportedCallableCandidate) -> Result<Self, String> {
        let interface = candidate.interface();
        if matches!(
            interface.effects().implementation(),
            hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::Atomic(_))
        ) {
            return Err(crate::expr::atomics::ATOMIC_REFERENCE_ERROR.into());
        }
        if candidate.callable_body().is_some()
            || matches!(
                interface.owner(),
                hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(_))
            )
        {
            let declaration = state
                .dependencies
                .as_ref()
                .expect("reference lookup has a dependency catalog")
                .callable_declaration(interface.declaration())
                .expect("a candidate retains its catalog declaration");
            let template = state.request_imported_generic_template(declaration)?;
            let (signature, _) = ImportedGenericTarget::Function(template).signature(state);
            return Ok(Self {
                candidate,
                implementation: ImportedReferenceImplementation::Template(template),
                signature: signature.signature,
                receiver: signature.receiver,
            });
        }
        let signature = state
            .imported_native_signature(&candidate)
            .map_err(|error| error.diagnostic("callable reference signature"))?;
        let receiver_key = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) => {
                Some(scoop_identity::SignatureTypeKey::Nominal(owner))
            }
            _ => interface.receiver().cloned(),
        };
        let receiver = receiver_key
            .as_ref()
            .map(|ty| state.imported_signature_type(ty))
            .transpose()
            .map_err(|error| error.diagnostic("callable reference receiver"))?;
        Ok(Self {
            candidate,
            implementation: ImportedReferenceImplementation::Dependency,
            signature,
            receiver,
        })
    }

    fn extension(&self) -> bool {
        self.candidate.interface().owner() == hir::PublicDeclarationOwnerV1::Extension
    }

    pub(super) fn layer(&self) -> &'static str {
        match self.candidate.interface().owner() {
            hir::PublicDeclarationOwnerV1::Extension => "extension candidate",
            hir::PublicDeclarationOwnerV1::Nominal(_) => "member candidate",
            _ => "dependency top-level candidate",
        }
    }

    pub(super) fn signature(&self, state: &Lowerer, name: &str) -> String {
        let binders = self
            .signature
            .owner_parameters
            .iter()
            .chain(&self.signature.callable_parameters)
            .cloned()
            .collect::<Vec<_>>();
        let type_parameters =
            render_type_parameters(state, &self.signature.callable_parameters, &binders);
        let receiver = self
            .receiver
            .map(|ty| format!("{}.", state.type_name_with_params(ty, &binders)))
            .unwrap_or_default();
        let parameters = self
            .signature
            .value_parameters
            .iter()
            .map(|parameter| {
                format!(
                    "{}: {}",
                    parameter.name,
                    state.type_name_with_params(parameter.ty, &binders)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let suspend = if self.is_suspend() { "suspend " } else { "" };
        format!(
            "{suspend}fun {receiver}{name}{type_parameters}({parameters}): {}",
            state.type_name_with_params(self.signature.return_type, &binders)
        )
    }

    fn is_suspend(&self) -> bool {
        self.candidate.interface().effects().execution() == scoop_identity::Effect::Suspend
    }

    pub(super) fn forwarding(&self, state: &mut Lowerer) -> OwnedDeclarationForwarding {
        let declaration = Self::resolve(state, self.candidate.clone())
            .expect("an applicable imported reference has a resolved declaration");
        let mut parameters = declaration
            .signature
            .value_parameters
            .iter()
            .map(|parameter| parameter.ty)
            .collect::<Vec<_>>();
        if declaration.extension() {
            parameters.insert(
                0,
                declaration.receiver.expect("an extension has a receiver"),
            );
        }
        DeclarationForwardingView::parameter_groups(
            &declaration.signature.owner_parameters,
            &declaration.signature.callable_parameters,
            &parameters,
        )
        .to_owned()
    }

    fn substitutions(&self, arguments: &[TypeId]) -> Vec<(hir::TypeParamId, TypeId)> {
        self.signature
            .owner_parameters
            .iter()
            .chain(&self.signature.callable_parameters)
            .map(|parameter| parameter.id)
            .zip(arguments.iter().copied())
            .collect()
    }

    pub(super) fn commit(
        self,
        state: &mut Lowerer,
        type_args: &[TypeId],
        receiver: Option<&hir::Expr>,
        ty: TypeId,
    ) -> Result<hir::CallableReferenceTarget, String> {
        let bindings = self.substitutions(type_args);
        let declared_receiver = self
            .receiver
            .map(|ty| state.instantiate_method_ty(ty, &bindings));
        let declaration = self.candidate.interface().clone();
        let extension = self.extension();
        if let Some(intrinsic) = self.candidate.normalized_intrinsic() {
            if intrinsic.requires_arithmetic_exception() {
                state
                    .prepare_arithmetic_exception_type()
                    .map_err(|error| error.diagnostic("integer reference exception type"))?;
            }
            let scoop_identity::CallableTemplateOrigin::Function(declaration) =
                declaration.declaration()
            else {
                unreachable!("a primitive member intrinsic is a non-generic function");
            };
            let receiver = state.adapt_to(
                receiver
                    .expect("an intrinsic member reference is bound")
                    .clone(),
                declared_receiver.expect("a primitive member has a receiver"),
            );
            return Ok(hir::CallableReferenceTarget::BoundIntrinsic {
                receiver: Box::new(receiver),
                declaration,
                intrinsic,
            });
        }
        let callee = match self.implementation {
            ImportedReferenceImplementation::Template(template) => {
                let arguments = match state.imported_generic_templates[template].declaration {
                    hir::ImportedCallableTemplateOrigin::Nominal { .. } => {
                        hir::ImportedCallableArguments::Method {
                            owner: declared_receiver
                                .expect("a nominal reference has a declaring receiver"),
                            method_arguments: type_args[self.signature.owner_parameters.len()..]
                                .to_vec(),
                        }
                    }
                    _ => hir::ImportedCallableArguments::Function(type_args.to_vec()),
                };
                hir::CallableTarget::Application(state.imported_generic_applications.alloc(
                    hir::ImportedGenericCallableApplication {
                        template,
                        arguments,
                    },
                ))
            }
            ImportedReferenceImplementation::Dependency => {
                let selected = match self.candidate {
                    ImportedCallableCandidate::Binding(candidate) => state
                        .select_imported_dependency_callable_use(*candidate)
                        .map(|(callee, _)| callee),
                    ImportedCallableCandidate::Declaration(candidate) => state
                        .select_imported_callable_declaration_use_with_kind(
                            *candidate,
                            MemberCallKind::Ordinary,
                        ),
                }
                .map_err(|error| {
                    format!("failed to select imported callable reference: {error}")
                })?;
                hir::CallableTarget::Dependency(selected)
            }
        };
        let target = if let Some(receiver) = receiver {
            let bound = if matches!(state.types[receiver.ty], Type::Param(_)) && !extension {
                let Type::Function(signature) = state.types[ty] else {
                    unreachable!("reference resolution produces a function type")
                };
                let signature = state.function_types[signature].clone();
                state.imported_bound_member_callee(
                    &declaration,
                    callee,
                    receiver.ty,
                    &signature.parameter_types,
                    signature.return_type,
                )?
            } else {
                None
            };
            let receiver = if bound.is_some() {
                receiver.clone()
            } else {
                state.adapt_to(
                    receiver.clone(),
                    declared_receiver.expect("a bound declaration has a receiver type"),
                )
            };
            if extension {
                hir::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(receiver),
                    callee,
                }
            } else {
                hir::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(receiver),
                    callee: bound.unwrap_or(hir::MethodCallee::Callable(callee)),
                }
            }
        } else {
            hir::CallableReferenceTarget::Named(callee)
        };
        Ok(target)
    }
}

impl Lowerer {
    pub(super) fn probe_imported_reference(
        &self,
        candidate: &ReferenceCandidate,
        receiver: Option<TypeId>,
        context: ReferenceResolutionContext<'_>,
    ) -> Result<Option<ApplicableReference>, ReferenceFailure> {
        let mut state = self.clone();
        let invalid = |reason| ReferenceFailure {
            signature: format!("dependency function `{}`", context.name),
            layer: "dependency candidate",
            reason,
        };
        let candidate = match candidate {
            ReferenceCandidate::Dependency(binding) => {
                ImportedCallableCandidate::Binding(Box::new(
                    state
                        .dependencies
                        .as_ref()
                        .expect("reference lookup has a dependency catalog")
                        .callable_candidate(binding)
                        .map_err(|error| {
                            invalid(format!("invalid imported callable reference: {error}"))
                        })?,
                ))
            }
            ReferenceCandidate::Member(declaration, _) => {
                ImportedCallableCandidate::Declaration(declaration.clone())
            }
            ReferenceCandidate::Local(..) => {
                unreachable!("local references use their local declaration view")
            }
        };
        let declaration =
            ImportedReferenceDeclaration::resolve(&mut state, candidate).map_err(invalid)?;
        let fail = |state: &Lowerer, reason| ReferenceFailure {
            signature: declaration.signature(state, context.name),
            layer: declaration.layer(),
            reason,
        };
        let extension = declaration.extension();
        let bound_receiver = match context.extension_mode {
            ReferenceExtensionMode::Exclude if extension => return Ok(None),
            ReferenceExtensionMode::Bound(_) if !extension => return Ok(None),
            ReferenceExtensionMode::Bound(actual) => Some((
                declaration.receiver.expect("an extension has a receiver"),
                actual,
            )),
            ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
        };
        let owner_arguments = if declaration.signature.owner_parameters.is_empty() {
            Vec::new()
        } else {
            let hir::PublicDeclarationOwnerV1::Nominal(owner) =
                declaration.candidate.interface().owner()
            else {
                unreachable!("only nominal members carry owner parameters")
            };
            receiver
                .and_then(|receiver| state.imported_member_owner_type(receiver, owner))
                .and_then(|ty| state.dependency_nominal_application(ty))
                .map(|(_, arguments)| arguments.to_vec())
                .unwrap_or_default()
        };
        let own_type_param_count = declaration.signature.callable_parameters.len();
        let application = state
            .solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                signature: &declaration.signature,
                owner_arguments: &owner_arguments,
                bound_receiver,
                unbound_receiver: if extension
                    && matches!(
                        context.extension_mode,
                        ReferenceExtensionMode::IncludeUnbound
                    ) {
                    declaration.receiver
                } else {
                    None
                },
                effects: CallableEffects {
                    is_suspend: declaration.is_suspend(),
                    attributes: declaration
                        .candidate
                        .interface()
                        .effects()
                        .function_attributes(),
                },
                expected_type: context.expected.map(|(ty, _)| *ty),
            })
            .map_err(|failure| {
                fail(
                    &state,
                    failure.describe(|constraint| {
                        state.render_imported_constraint_failure(
                            &declaration.signature.owner_parameters,
                            &declaration.signature.callable_parameters,
                            constraint,
                        )
                    }),
                )
            })?;
        Ok(Some(ApplicableReference {
            state: Box::new(state),
            declaration: ReferenceDeclaration::Imported(declaration),
            type_args: application.type_args,
            ty: application.ty,
            own_type_param_count,
            receiver: None,
        }))
    }
}
