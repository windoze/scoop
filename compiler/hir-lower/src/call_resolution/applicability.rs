//! Constraint construction for one callable candidate.

use scoop_hir as hir;

use super::arguments::{CandidateArgumentMap, SourceInputId};
use super::candidates::{CallableView, ReceiverShape};
use super::constraints::{
    Constraint, ConstraintFailure, ConstraintOrigin, InferenceSession, TypeTerm,
};
use crate::Lowerer;

#[derive(Debug, Clone)]
pub(crate) struct CallableApplicabilityInput<'a> {
    pub(crate) view: &'a CallableView,
    pub(crate) argument_map: &'a CandidateArgumentMap,
    pub(crate) owner_arguments: &'a [hir::TypeId],
    pub(crate) explicit_arguments: &'a [hir::TypeId],
    pub(crate) receiver_type: Option<hir::TypeId>,
    pub(crate) argument_types: &'a [Option<hir::TypeId>],
}

impl Lowerer {
    pub(crate) fn solve_callable_applicability(
        &mut self,
        input: CallableApplicabilityInput<'_>,
    ) -> Result<Vec<hir::TypeId>, ConstraintFailure> {
        let CallableApplicabilityInput {
            view,
            argument_map,
            owner_arguments,
            explicit_arguments,
            receiver_type,
            argument_types,
        } = input;
        debug_assert_eq!(view.owner_parameters.len(), owner_arguments.len());
        debug_assert!(
            explicit_arguments.is_empty()
                || view.callable_parameters.len() == explicit_arguments.len()
        );
        debug_assert_eq!(argument_map.parameters.len(), argument_types.len());

        let mut session = InferenceSession::new();
        let environment =
            session.add_environment(&view.owner_parameters, &view.callable_parameters);

        let owner_variables = session.owner_variables(environment).to_vec();
        for (&variable, &argument) in owner_variables.iter().zip(owner_arguments) {
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                ConstraintOrigin::Receiver,
            );
        }
        let callable_variables = session.callable_variables(environment).to_vec();
        for (index, (&variable, &argument)) in callable_variables
            .iter()
            .zip(explicit_arguments)
            .enumerate()
        {
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                ConstraintOrigin::ExplicitTypeArgument(
                    u32::try_from(index).expect("explicit type argument index exceeds u32"),
                ),
            );
        }

        self.add_declaration_bounds(
            &mut session,
            view.owner_parameters
                .iter()
                .chain(&view.callable_parameters),
        );

        if let ReceiverShape::Extension(expected) = view.receiver {
            let actual = receiver_type.expect("extension applicability has a receiver type");
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(actual), TypeTerm::Type(expected)),
                ConstraintOrigin::Receiver,
            );
        } else {
            debug_assert!(receiver_type.is_none());
        }

        for input in &argument_map.parameters {
            let source_index = input.input.index();
            let Some(actual) = argument_types[source_index] else {
                continue;
            };
            let expected = view.value_parameters[input.parameter.index()].ty;
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(actual), TypeTerm::Type(expected)),
                ConstraintOrigin::Argument(SourceInputId::from_index(source_index)),
            );
        }

        let solution = self.solve_constraints(&session)?;
        let arguments = solution.arguments_for(&session, environment);
        Ok(arguments
            .owner
            .into_iter()
            .chain(arguments.callable)
            .collect())
    }

    fn add_declaration_bounds<'a>(
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
            for bound in parameter.interface_bounds() {
                let interface = self.interface_applications[bound.application].canonical_type;
                session.push(
                    Constraint::Implements(variable, TypeTerm::Type(interface)),
                    ConstraintOrigin::TypeParameterBound(parameter.id),
                );
            }
        }
    }
}
