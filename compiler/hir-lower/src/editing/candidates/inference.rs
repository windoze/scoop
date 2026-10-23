use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};
use crate::call_resolution::arguments::{ArgumentShape, ReceiverInput, SuppliedArgumentMap};
use crate::call_resolution::candidates::{ValueParameterCalling, VarargOmission};
use crate::call_resolution::contextual::{
    ArgumentExpression, ArgumentInferenceFailureKind, ArgumentInferenceInput,
};
use crate::declaration_inference::DeclarationResult;

pub(crate) struct CandidateInput<'a, D> {
    pub signature: DeclarationSignature<D>,
    pub type_arguments: DeclarationTypeArguments<'a>,
    pub receiver: Option<(hir::TypeId, hir::TypeId)>,
    pub argument_mode: ArgumentMode,
    pub result: V<hir::TypeId>,
    pub named_parameters: bool,
    pub explicit_arguments: Option<&'a [crate::expr::ResolvedCallTypeArgument]>,
}

impl Lowerer {
    pub(crate) fn editing_signature_candidate<D: Copy>(
        &self,
        call: &ast::RecoveredCall,
        input: CandidateInput<'_, D>,
    ) -> Candidate {
        let CandidateInput {
            signature,
            type_arguments,
            receiver,
            argument_mode,
            result,
            named_parameters,
            explicit_arguments,
        } = input;
        let explicit_count = match type_arguments {
            DeclarationTypeArguments::Callable { .. } => signature.callable_parameters.len(),
            DeclarationTypeArguments::Nominal { .. } => signature.owner_parameters.len(),
        };
        let supplied_types = explicit_arguments.map_or(call.type_args.len(), <[_]>::len);
        if supplied_types != 0 && supplied_types != explicit_count {
            return Ok(None);
        }
        let source = arguments(call).collect::<Vec<_>>();
        let shapes = source
            .iter()
            .map(|argument| ArgumentShape {
                name: match &argument.name {
                    ast::CallArgumentName::Named(name) => Some(name.text.as_str()),
                    ast::CallArgumentName::Positional | ast::CallArgumentName::TrailingLambda => {
                        None
                    }
                },
                spread: matches!(argument.spread, ast::SpreadSyntax::Spread(_)),
                trailing: matches!(argument.name, ast::CallArgumentName::TrailingLambda),
            })
            .collect::<Vec<_>>();
        let Ok(map) = SuppliedArgumentMap::declaration(
            &signature.value_parameters,
            argument_mode,
            &shapes,
            if receiver.is_some() {
                ReceiverInput::Present
            } else {
                ReceiverInput::Absent
            },
        ) else {
            return Ok(None);
        };
        let patterns = map.inference_patterns(&signature.value_parameters);
        // Never renumber a later argument across an unknown source operation.
        let expressions = source
            .iter()
            .map_while(|argument| match &argument.value {
                ast::RecoveredArgumentValue::Complete(expression) => {
                    Some(ArgumentExpression::Source(expression))
                }
                ast::RecoveredArgumentValue::Unavailable(_)
                | ast::RecoveredArgumentValue::CallableLiteral(_) => None,
            })
            .collect::<Vec<_>>();
        let mut state = self.clone();
        let explicit = match explicit_arguments {
            Some(arguments) => arguments.to_vec(),
            None => state
                .resolve_call_type_args(&call.type_args)
                .ok_or(U::BodyFailure)?,
        };
        let (mut session, environment) =
            state.declaration_applicability_session(DeclarationApplicabilityInput {
                availability: signature.availability,
                owner_parameters: &signature.owner_parameters,
                callable_parameters: &signature.callable_parameters,
                type_arguments,
                explicit_arguments: &explicit,
                bound_receiver: receiver,
            });
        let parameters = signature
            .owner_parameters
            .iter()
            .chain(&signature.callable_parameters)
            .cloned()
            .collect::<Vec<_>>();
        let ids = parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let facts = state.infer_contextual_argument_facts(ArgumentInferenceInput {
            expressions: &expressions,
            patterns: &patterns[..expressions.len()],
            parameters: &ids,
            session: &mut session,
            environment,
            result: DeclarationResult::Known(signature.return_type),
            expected_result: None,
            forced_hint: None,
        });
        let (values, hints) = match facts {
            Ok(facts) => (facts.values, Some(facts.hints)),
            Err(failure) => match failure.kind {
                ArgumentInferenceFailureKind::Constraint(_) => return Ok(None),
                ArgumentInferenceFailureKind::Declaration { .. } => {
                    return Err(U::SignatureFailure);
                }
                ArgumentInferenceFailureKind::Expression(error) => {
                    let ast::RecoveredArgumentValue::Complete(expression) =
                        &source[error.source_index].value
                    else {
                        unreachable!("only complete expressions are checked");
                    };
                    if !error.context_dependent
                        && self.expr_context_requirement(expression)
                            == crate::expr::ExpressionContextRequirement::Independent
                    {
                        return Err(U::BodyFailure);
                    }
                    (failure.arguments, None)
                }
            },
        };
        let owner_arguments = match type_arguments {
            DeclarationTypeArguments::Callable { owner_arguments } => owner_arguments,
            DeclarationTypeArguments::Nominal {
                expected_arguments, ..
            } => expected_arguments.unwrap_or_default(),
        };
        let substitutions = signature
            .owner_parameters
            .iter()
            .zip(owner_arguments)
            .map(|(parameter, &ty)| (parameter.id, ty))
            .collect::<Vec<_>>();
        let mut display = signature.clone();
        display.substitute_types(&mut state, &substitutions);
        let mut display_parameters = parameters;
        for parameter in &state.type_params_in_scope {
            if !display_parameters
                .iter()
                .any(|item| item.id == parameter.id)
            {
                display_parameters.push(parameter.clone());
            }
        }
        let format_type = |ty| state.type_name_with_params(ty, &display_parameters);
        let signature = hir::EditingCallCandidate {
            parameters: display
                .value_parameters
                .iter()
                .map(|parameter| {
                    let (ty, vararg, has_default) = match parameter.calling {
                        ValueParameterCalling::Required => (parameter.ty, false, false),
                        ValueParameterCalling::Default(_) => (parameter.ty, false, true),
                        ValueParameterCalling::Vararg {
                            element_type,
                            omission,
                            ..
                        } => (
                            element_type,
                            true,
                            matches!(omission, VarargOmission::Default(_)),
                        ),
                    };
                    hir::EditingCandidateParameter {
                        name: named_parameters.then(|| parameter.name.clone()),
                        type_label: format_type(ty),
                        vararg,
                        has_default,
                    }
                })
                .collect(),
            result: match result {
                V::Known(_) => V::Known(format_type(display.return_type)),
                V::Unavailable(reason) => V::Unavailable(reason),
            },
            source_parameters: map
                .source_order
                .iter()
                .map(|source| map.source_binding(*source).0.index())
                .collect(),
            missing_parameters: map
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(index, input)| input.is_none().then_some(index))
                .collect(),
            unchecked_arguments: (0..source.len())
                .filter(|&index| values.get(index).is_none_or(Option::is_none))
                .collect(),
        };
        let argument = hints
            .filter(|_| values.iter().all(Option::is_some))
            .and_then(|hints| {
                let index = expressions.len();
                let ast::RecoveredArgumentValue::CallableLiteral(body) = source.get(index)?.value
                else {
                    return None;
                };
                let expected = state.callable_argument_expectation(patterns[index].ty, &hints)?;
                if matches!(&expected, crate::expr::ExpressionExpectation::Type(ty) if !matches!(state.types[*ty], hir::Type::Function(_))) {
                    return None;
                }
                Some(Box::new(ArgumentContext {
                    body,
                    expected,
                    state,
                }))
            });
        Ok(Some(CallCandidateAnalysis {
            signature,
            argument,
        }))
    }
}
