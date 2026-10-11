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
mod signature;

#[derive(Clone)]
pub(super) enum ConstructorReferenceSource {
    Local(NominalConstructorSource),
    Imported(Box<hir::ImportedCallableDeclaration>),
}

#[derive(Clone)]
pub(super) struct ConstructorReferenceCandidate {
    source: ConstructorReferenceSource,
    fixed: Option<TypeId>,
    alias: Option<crate::aliases::AliasTemplate>,
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
    alias_arguments: Option<Vec<TypeId>>,
    owner_arguments: Vec<TypeId>,
    span: Span,
}

impl ConstructorReferenceDeclaration {
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
        let mut declaration = Self::resolve(state, self.candidate.clone(), self.span)
            .expect("an applicable constructor reference retains its declaration");
        let bindings = declaration
            .signature
            .owner_parameters
            .iter()
            .map(|parameter| parameter.id)
            .zip(declaration.owner_arguments)
            .collect::<Vec<_>>();
        declaration.signature.owner_parameters.clear();
        declaration.signature.substitute_types(state, &bindings);
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
        let arguments = if let Some(targets) = &self.alias_arguments {
            let bindings = self
                .signature
                .owner_parameters
                .iter()
                .chain(&self.signature.callable_parameters)
                .map(|parameter| parameter.id)
                .zip(arguments.iter().copied())
                .collect::<Vec<_>>();
            targets
                .iter()
                .map(|ty| state.instantiate_method_ty(*ty, &bindings))
                .collect()
        } else {
            self.fixed_arguments.unwrap_or_else(|| arguments.to_vec())
        };
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
        let own_type_param_count = candidate
            .alias
            .as_ref()
            .map_or(declaration.signature.callable_parameters.len(), |alias| {
                alias.parameters.len() - alias.outer_parameter_count
            });
        let application = state
            .solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                signature: &declaration.signature,
                owner_arguments: &declaration.owner_arguments,
                bound_receiver: None,
                unbound_receiver: None,
                effects: declaration.effects,
                expected_type: context.expected.map(|(ty, _)| *ty),
            })
            .map_err(|failure| {
                fail(failure.describe(|constraint| {
                    state.render_imported_constraint_failure(
                        &declaration.signature.owner_parameters,
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
