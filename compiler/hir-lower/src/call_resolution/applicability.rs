//! Constraint construction for one callable candidate.

use scoop_hir as hir;

use super::arguments::CandidateArgumentMap;
use super::candidates::NominalConstructorView;
use super::constraints::{
    Constraint, ConstraintFailure, ConstraintOrigin, InferenceSession, NominalApplication, TypeTerm,
};
use crate::Lowerer;
use crate::expr::ResolvedCallTypeArgument;

mod references;
pub(crate) use references::CallableReferenceApplicabilityInput;

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeclarationApplicabilityInput<'a> {
    pub(crate) owner_parameters: &'a [hir::TypeParamDecl],
    pub(crate) callable_parameters: &'a [hir::TypeParamDecl],
    pub(crate) type_arguments: DeclarationTypeArguments<'a>,
    pub(crate) explicit_arguments: &'a [ResolvedCallTypeArgument],
    pub(crate) bound_receiver: Option<(hir::TypeId, hir::TypeId)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeclarationTypeArguments<'a> {
    Callable {
        owner_arguments: &'a [hir::TypeId],
    },
    Nominal {
        template: hir::SourceNominalId,
        expected_arguments: Option<&'a [hir::TypeId]>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct NominalApplicabilityInput<'a> {
    pub(crate) view: &'a NominalConstructorView,
    pub(crate) argument_map: &'a CandidateArgumentMap,
    pub(crate) explicit_arguments: &'a [ResolvedCallTypeArgument],
    pub(crate) expected_arguments: Option<&'a [hir::TypeId]>,
    pub(crate) argument_types: &'a [Option<hir::TypeId>],
}

impl Lowerer {
    pub(crate) fn declaration_applicability_session(
        &self,
        input: DeclarationApplicabilityInput<'_>,
    ) -> (InferenceSession, super::constraints::InferenceEnvironmentId) {
        let DeclarationApplicabilityInput {
            owner_parameters,
            callable_parameters,
            type_arguments,
            explicit_arguments,
            bound_receiver,
        } = input;
        let mut session = InferenceSession::new();
        let environment = session.add_environment(owner_parameters, callable_parameters);
        let owner_variables = session.owner_variables(environment).to_vec();
        let explicit_variables: Vec<super::constraints::InferenceVariableId> = match type_arguments
        {
            DeclarationTypeArguments::Callable { owner_arguments } => {
                debug_assert_eq!(owner_parameters.len(), owner_arguments.len());
                for (&variable, &argument) in owner_variables.iter().zip(owner_arguments) {
                    session.push(
                        Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                        ConstraintOrigin::Receiver,
                    );
                }
                session
                    .callable_variables(environment)
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect()
            }
            DeclarationTypeArguments::Nominal {
                template,
                expected_arguments,
            } => {
                debug_assert!(callable_parameters.is_empty());
                if let Some(expected_arguments) = expected_arguments {
                    debug_assert_eq!(owner_parameters.len(), expected_arguments.len());
                    for (&variable, &argument) in owner_variables.iter().zip(expected_arguments) {
                        session.push(
                            Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                            ConstraintOrigin::ExpectedResult,
                        );
                    }
                }
                session.push(
                    Constraint::ConcreteApplication(NominalApplication {
                        template,
                        arguments: owner_variables
                            .iter()
                            .copied()
                            .map(TypeTerm::from)
                            .collect(),
                    }),
                    ConstraintOrigin::Declaration,
                );
                owner_variables.into_iter().map(Into::into).collect()
            }
        };
        debug_assert!(
            explicit_arguments.is_empty() || explicit_variables.len() == explicit_arguments.len()
        );
        for (index, (&variable, &argument)) in explicit_variables
            .iter()
            .zip(explicit_arguments)
            .enumerate()
        {
            let ResolvedCallTypeArgument::Explicit { ty: argument, .. } = argument else {
                continue;
            };
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                ConstraintOrigin::ExplicitTypeArgument(
                    u32::try_from(index).expect("explicit type argument index exceeds u32"),
                ),
            );
        }
        self.add_declaration_bounds(
            &mut session,
            owner_parameters.iter().chain(callable_parameters),
        );
        if let Some((expected, actual)) = bound_receiver {
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(actual), TypeTerm::Type(expected)),
                ConstraintOrigin::Receiver,
            );
        }
        (session, environment)
    }

    pub(crate) fn solve_nominal_applicability(
        &mut self,
        input: NominalApplicabilityInput<'_>,
    ) -> Result<Vec<hir::TypeId>, ConstraintFailure> {
        let (session, environment) = self.nominal_applicability_session(input);
        let solution = self.solve_constraints(&session)?;
        Ok(solution.arguments_for(&session, environment).owner)
    }

    pub(crate) fn nominal_applicability_session(
        &self,
        input: NominalApplicabilityInput<'_>,
    ) -> (InferenceSession, super::constraints::InferenceEnvironmentId) {
        let NominalApplicabilityInput {
            view,
            argument_map,
            explicit_arguments,
            expected_arguments,
            argument_types,
        } = input;
        debug_assert_eq!(argument_map.source_order.len(), argument_types.len());
        let (mut session, environment) =
            self.declaration_applicability_session(DeclarationApplicabilityInput {
                owner_parameters: &view.signature.owner_parameters,
                callable_parameters: &[],
                type_arguments: DeclarationTypeArguments::Nominal {
                    template: self
                        .nominal_application(view.signature.return_type)
                        .expect("a nominal candidate retains its full result application")
                        .template,
                    expected_arguments,
                },
                explicit_arguments,
                bound_receiver: None,
            });
        for (index, (pattern, actual)) in argument_map
            .inference_patterns(&view.signature.value_parameters)
            .into_iter()
            .zip(argument_types)
            .enumerate()
        {
            if let Some(actual) = actual {
                pattern.constrain(&mut session, index, *actual);
            }
        }

        (session, environment)
    }

    pub(crate) fn add_declaration_bounds<'a>(
        &self,
        session: &mut InferenceSession,
        parameters: impl Iterator<Item = &'a hir::TypeParamDecl>,
    ) {
        for parameter in parameters {
            let variable = session
                .variable_for(parameter.id)
                .expect("declaration parameter has a fresh inference variable");
            session.push(
                Constraint::Kind(variable, parameter.kind()),
                ConstraintOrigin::TypeParameterBound(parameter.id),
            );
            for bound in parameter.nominal_bounds_in_source_order() {
                let constraint = match bound {
                    hir::NominalBoundRef::Class(bound) => {
                        Constraint::ClassBound(variable, TypeTerm::Type(bound.ty))
                    }
                    hir::NominalBoundRef::Interface(bound) => {
                        Constraint::Implements(variable, TypeTerm::Type(bound.ty))
                    }
                };
                session.push(
                    constraint,
                    ConstraintOrigin::TypeParameterBound(parameter.id),
                );
            }
        }
    }
}
