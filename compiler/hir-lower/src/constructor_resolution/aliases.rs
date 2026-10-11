use super::*;
use crate::call_resolution::applicability::DeclarationTypeArguments;
use crate::call_resolution::contextual::{ArgumentExpression, ArgumentInferenceFailureKind};
use crate::call_resolution::probe::CallInferenceInput;
use crate::declaration_inference::DeclarationResult;

impl Lowerer {
    pub(crate) fn probe_named_alias_constructor(
        &self,
        view: NominalConstructorView,
        alias: &crate::aliases::AliasTemplate,
        call: NominalConstructorCall<'_>,
        expected: Option<TypeId>,
    ) -> Result<NamedNominalProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let enclosing =
            match state.constructor_enclosing_instance(view.target, call.enclosing, call.span) {
                Ok(receiver) => receiver,
                Err(()) => return Err(Box::new(state)),
            };
        let Some(mapped) = state.alias_constructor_signature(
            alias,
            &view.signature,
            call.explicit_type_args,
            call.span,
        ) else {
            return Err(Box::new(state));
        };
        let mut mapped_view = view.clone();
        let bound_receiver = enclosing.as_ref().map(|receiver| {
            let NominalConstructorSource::Class(constructor) = view.target else {
                unreachable!()
            };
            let parameter = state.class_constructors[constructor]
                .context_parameter
                .as_ref()
                .expect("an inner constructor has an outer parameter")
                .ty;
            (
                state.instantiate_method_ty(parameter, &mapped.target_bindings),
                receiver.ty,
            )
        });
        mapped_view.signature = mapped.signature;
        let argument_map = match CandidateArgumentMap::source_nominal(&mapped_view, call.arguments)
        {
            Ok(map) => map,
            Err(error) => {
                state.error(call.span, error.describe());
                return Err(Box::new(state));
            }
        };
        let expressions = call
            .arguments
            .iter()
            .map(|argument| ArgumentExpression::Source(&argument.expression))
            .collect::<Vec<_>>();
        let before = state.diagnostics.len();
        let inferred = match state.infer_call_arguments(CallInferenceInput {
            signature: &mapped_view.signature,
            result: DeclarationResult::Known(mapped_view.signature.return_type),
            argument_map: &argument_map,
            type_arguments: DeclarationTypeArguments::Callable {
                owner_arguments: &[],
            },
            explicit_arguments: &mapped.explicit,
            bound_receiver,
            expressions: &expressions,
            expected_result: expected,
            forced_hint: None,
        }) {
            Ok(inferred) => inferred,
            Err(failure) => {
                if let ArgumentInferenceFailureKind::Constraint(constraint) = failure.kind {
                    let reason = state.render_imported_constraint_failure(
                        &[],
                        &mapped_view.signature.callable_parameters,
                        &constraint,
                    );
                    state.error(call.span, format!("typealias constructor: {reason}"));
                }
                return Err(Box::new(state));
            }
        };
        if state.diagnostics.len() != before {
            return Err(Box::new(state));
        }
        let parameter_types =
            argument_map.forwarding_parameter_types(&view.signature.value_parameters);
        let type_args = mapped
            .target_bindings
            .iter()
            .map(|(_, ty)| state.instantiate_method_ty(*ty, &inferred.bindings))
            .collect();
        Ok(NamedNominalProbe {
            alias: Some(alias.clone()),
            comparison_owners: mapped_view.signature.callable_parameters,
            comparison_parameters: argument_map
                .forwarding_parameter_types(&mapped_view.signature.value_parameters),
            candidate: ApplicableConstructor {
                enclosing,
                source: view.target,
                view,
                argument_map,
                parameter_types,
                inferred: NominalArguments {
                    type_args,
                    args: inferred.values,
                    argument_sinks: inferred.sinks,
                },
                state: Box::new(state),
            },
        })
    }
}
