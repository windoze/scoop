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

mod adapter;
mod enclosing;
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
    enclosing: Option<hir::Expr>,
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
    enclosing: Option<(hir::Expr, TypeId)>,
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
                bound_receiver: declaration
                    .enclosing
                    .as_ref()
                    .map(|(actual, formal)| (*formal, actual.ty)),
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
            declaration: ReferenceDeclaration::Constructor(Box::new(declaration)),
            type_args: application.type_args,
            ty: application.ty,
            own_type_param_count,
            receiver: None,
        }))
    }
}
