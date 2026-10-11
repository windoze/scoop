use super::*;
use crate::call_resolution::applicability::CallableReferenceApplicabilityInput;
use crate::call_resolution::candidates::{
    CallableEffects, DeclarationSignature, NominalConstructorSource, ValueParameter,
    ValueParameterCalling,
};
use crate::call_resolution::specificity::{DeclarationForwardingView, OwnedDeclarationForwarding};
use crate::expr::named_calls::imported_dependency::{
    ImportedCallableCandidate, ImportedGenericTarget,
};
use hir::ImportedCallableSource;
use resolution::{ApplicableReference, ReferenceDeclaration, ReferenceFailure};

mod lookup;

#[derive(Clone)]
pub(super) enum ConstructorReferenceSource {
    Local(NominalConstructorSource),
    Imported(Box<hir::ImportedCallableDeclaration>),
}

#[derive(Clone)]
pub(super) struct ConstructorReferenceCandidate {
    source: ConstructorReferenceSource,
    fixed: Option<TypeId>,
}

enum ConstructorReferenceTarget {
    Local(NominalConstructorSource),
    Imported(hir::ImportedConstructorTemplateId),
    Dependency(Box<hir::ImportedCallableDeclaration>),
}

pub(super) struct ConstructorReferenceDeclaration {
    candidate: ConstructorReferenceCandidate,
    target: ConstructorReferenceTarget,
    signature: DeclarationSignature,
    effects: CallableEffects,
    fixed_arguments: Option<Vec<TypeId>>,
    span: Span,
}

impl ConstructorReferenceDeclaration {
    fn resolve(
        state: &mut Lowerer,
        candidate: ConstructorReferenceCandidate,
        span: Span,
    ) -> Result<Self, String> {
        let (target, mut signature, safety) = match &candidate.source {
            ConstructorReferenceSource::Local(source) => {
                let safety = match source {
                    NominalConstructorSource::Class(id) => state.class_constructors[*id].safety,
                    NominalConstructorSource::Struct(id) => state.struct_constructors[*id].safety,
                    _ => hir::Safety::Safe,
                };
                (
                    ConstructorReferenceTarget::Local(*source),
                    state.nominal_constructor_view(*source, span).signature,
                    safety,
                )
            }
            ConstructorReferenceSource::Imported(declaration) => {
                let safety = declaration
                    .interface()
                    .effects()
                    .function_attributes()
                    .safety;
                let (target, source) = if matches!(
                    declaration.interface().owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(
                        _
                    ))
                ) {
                    let template =
                        state.request_imported_constructor_template(*declaration.clone())?;
                    let (loaded, _) = ImportedGenericTarget::Constructor(template).signature(state);
                    (
                        ConstructorReferenceTarget::Imported(template),
                        loaded.signature,
                    )
                } else {
                    let candidate = ImportedCallableCandidate::Declaration(declaration.clone());
                    let signature = state
                        .imported_native_signature(&candidate)
                        .map_err(|error| error.diagnostic("constructor reference signature"))?;
                    (
                        ConstructorReferenceTarget::Dependency(declaration.clone()),
                        signature,
                    )
                };
                (
                    target,
                    DeclarationSignature {
                        owner_parameters: source.owner_parameters,
                        callable_parameters: source.callable_parameters,
                        value_parameters: source
                            .value_parameters
                            .into_iter()
                            .map(|parameter| ValueParameter {
                                name: parameter.name,
                                ty: parameter.ty,
                                calling: ValueParameterCalling::Required,
                            })
                            .collect(),
                        return_type: source.return_type,
                    },
                    safety,
                )
            }
        };
        let fixed_arguments = candidate.fixed.map(|ty| {
            state
                .nominal_application(ty)
                .expect("a fixed constructor alias has a nominal application")
                .arguments
        });
        if let Some(arguments) = &fixed_arguments {
            let bindings = signature
                .owner_parameters
                .iter()
                .map(|parameter| parameter.id)
                .zip(arguments.iter().copied())
                .collect::<Vec<_>>();
            for parameter in &mut signature.value_parameters {
                parameter.ty = state.instantiate_method_ty(parameter.ty, &bindings);
            }
            signature.return_type = state.instantiate_method_ty(signature.return_type, &bindings);
            signature.owner_parameters.clear();
        }
        // Constructor host parameters are inferred at the reference creation site.
        // Their existing typed identities remain the declaration's binders.
        signature
            .callable_parameters
            .append(&mut signature.owner_parameters);
        let attributes = hir::FunctionAttributes {
            safety,
            ..Default::default()
        };
        Ok(Self {
            candidate,
            target,
            signature,
            effects: CallableEffects {
                is_suspend: false,
                attributes,
            },
            fixed_arguments,
            span,
        })
    }

    pub(super) fn signature(&self, state: &Lowerer, name: &str) -> String {
        let binders = &self.signature.callable_parameters;
        let parameters = self
            .signature
            .value_parameters
            .iter()
            .map(|parameter| state.type_name_with_params(parameter.ty, binders))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "constructor {name}({parameters}): {}",
            state.type_name_with_params(self.signature.return_type, binders)
        )
    }

    pub(super) fn forwarding(&self, state: &mut Lowerer) -> OwnedDeclarationForwarding {
        let declaration = Self::resolve(state, self.candidate.clone(), self.span)
            .expect("an applicable constructor reference retains its declaration");
        let parameters = declaration
            .signature
            .value_parameters
            .iter()
            .map(|parameter| parameter.ty)
            .collect::<Vec<_>>();
        DeclarationForwardingView::parameter_groups(
            &declaration.signature.owner_parameters,
            &declaration.signature.callable_parameters,
            &parameters,
        )
        .to_owned()
    }

    pub(super) fn commit(
        self,
        state: &mut Lowerer,
        arguments: &[TypeId],
        ty: TypeId,
    ) -> Result<hir::Expr, String> {
        let Type::Function(function) = state.types[ty] else {
            unreachable!("a constructor reference has a complete function type");
        };
        let signature = state.function_types[function].clone();
        let arguments = self.fixed_arguments.unwrap_or_else(|| arguments.to_vec());
        let span = self.span;
        state
            .lower_synthesized_lambda(
                false,
                &signature.parameter_types,
                signature.return_type,
                span,
                |state, args| {
                    let value = match self.target {
                        ConstructorReferenceTarget::Local(source) => state.finish_named_nominal(
                            crate::constructor_resolution::ResolvedNominalConstructor {
                                source,
                                type_args: arguments,
                                args,
                            },
                            span,
                        )?,
                        ConstructorReferenceTarget::Dependency(declaration) => {
                            let callee = match state.select_imported_callable_declaration_use_with_kind(*declaration, MemberCallKind::Ordinary) {
                                Ok(callee) => callee,
                                Err(error) => { state.error(span, format!("failed to select imported constructor reference: {error}")); return None; }
                            };
                            hir::Expr {
                                kind: ExprKind::Call { callee: hir::CallableTarget::Dependency(callee), binding: None, args, receiver: hir::SourceCallReceiver::NoReceiver },
                                ty: signature.return_type,
                                span,
                                origin: state.expression_origin(span),
                            }
                        }
                        ConstructorReferenceTarget::Imported(template) => {
                            let kind = match state
                                .constructor_template_application(template, signature.return_type)
                            {
                                hir::ConstructorApplicationRef::Class(constructor) => {
                                    ExprKind::ClassInit { constructor, args }
                                }
                                hir::ConstructorApplicationRef::Struct(constructor) => {
                                    ExprKind::StructInit { constructor, args }
                                }
                            };
                            hir::Expr {
                                kind,
                                ty: signature.return_type,
                                span,
                                origin: state.expression_origin(span),
                            }
                        }
                    };
                    Some(crate::stmt::ValueBlock {
                        statements: Vec::new(),
                        value: Some(value),
                    })
                },
            )
            .ok_or_else(|| "constructor reference adapter could not be typed".to_string())
    }
}

impl Lowerer {
    pub(super) fn probe_constructor_reference(
        &self,
        candidate: &ConstructorReferenceCandidate,
        context: ReferenceResolutionContext<'_>,
    ) -> Result<Option<ApplicableReference>, ReferenceFailure> {
        let mut state = self.clone();
        let fail = |reason| ReferenceFailure {
            signature: format!("constructor `{}`", context.name),
            layer: "constructor candidate",
            reason,
        };
        let declaration =
            ConstructorReferenceDeclaration::resolve(&mut state, candidate.clone(), context.span)
                .map_err(fail)?;
        let own_type_param_count = declaration.signature.callable_parameters.len();
        let application = state
            .solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                signature: &declaration.signature,
                owner_arguments: &[],
                bound_receiver: None,
                unbound_receiver: None,
                effects: declaration.effects,
                expected_type: context.expected.map(|(ty, _)| *ty),
            })
            .map_err(|failure| {
                fail(failure.describe(|constraint| {
                    state.render_imported_constraint_failure(
                        &[],
                        &declaration.signature.callable_parameters,
                        constraint,
                    )
                }))
            })?;
        Ok(Some(ApplicableReference {
            state: Box::new(state),
            declaration: ReferenceDeclaration::Constructor(declaration),
            type_args: application.type_args,
            ty: application.ty,
            own_type_param_count,
            receiver: None,
        }))
    }
}
