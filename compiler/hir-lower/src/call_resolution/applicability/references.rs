//! Exact managed and native reference signatures share declaration constraints.

use super::*;
use crate::Type;
use crate::call_resolution::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape,
};

impl Lowerer {
    pub(crate) fn check_concrete_callable_signature(
        &mut self,
        category: CallableCategory,
        is_suspend: bool,
        parameter_types: &[hir::TypeId],
        return_type: hir::TypeId,
        expected_type: hir::TypeId,
    ) -> Result<(), ConstraintFailure> {
        let mut session = InferenceSession::new();
        let shape = CallableShape {
            category,
            is_suspend,
            parameters: parameter_types
                .iter()
                .copied()
                .map(TypeTerm::Rigid)
                .map(CallableParameter::Explicit)
                .collect(),
            return_type: CallableReturn::Explicit(TypeTerm::Rigid(return_type)),
        };
        let expected_signature = match (category, &self.types[expected_type]) {
            (CallableCategory::Managed, Type::Function(signature))
            | (CallableCategory::Native, Type::FunPtr(signature)) => {
                Some(self.function_types[*signature].clone())
            }
            _ => None,
        };
        if let Some(expected) = expected_signature
            && expected.parameter_types.len() == parameter_types.len()
        {
            for (&parameter, &expected) in parameter_types.iter().zip(&expected.parameter_types) {
                session.push(
                    Constraint::Equal(TypeTerm::Rigid(parameter), TypeTerm::Rigid(expected)),
                    ConstraintOrigin::ExpectedResult,
                );
            }
            session.push(
                Constraint::Equal(
                    TypeTerm::Rigid(return_type),
                    TypeTerm::Rigid(expected.return_type),
                ),
                ConstraintOrigin::ExpectedResult,
            );
        }
        session.push(
            Constraint::CallableShape(shape, TypeTerm::Rigid(expected_type)),
            ConstraintOrigin::ExpectedResult,
        );
        self.solve_constraints(&session).map(|_| ())
    }

    pub(crate) fn solve_callable_reference_applicability(
        &mut self,
        input: CallableReferenceApplicabilityInput<'_>,
    ) -> Result<Vec<hir::TypeId>, ConstraintFailure> {
        let CallableReferenceApplicabilityInput {
            owner_parameters,
            callable_parameters,
            owner_arguments,
            bound_receiver,
            parameter_types,
            return_type,
            is_suspend,
            expected_type,
        } = input;
        let (mut session, environment) =
            self.declaration_applicability_session(DeclarationApplicabilityInput {
                owner_parameters,
                callable_parameters,
                type_arguments: DeclarationTypeArguments::Callable { owner_arguments },
                explicit_arguments: &[],
                bound_receiver,
            });

        let shape = CallableShape {
            category: CallableCategory::Managed,
            is_suspend,
            parameters: parameter_types
                .iter()
                .copied()
                .map(TypeTerm::Type)
                .map(CallableParameter::Explicit)
                .collect(),
            return_type: CallableReturn::Explicit(TypeTerm::Type(return_type)),
        };
        // A declaration reference denotes its exact instantiated signature.
        // Function variance is represented by later value coercions, not by
        // choosing a different declaration instantiation here.
        if let Type::Function(expected) = self.types[expected_type] {
            let expected = self.function_types[expected].clone();
            if parameter_types.len() == expected.parameter_types.len() {
                for (&parameter, &expected) in parameter_types.iter().zip(&expected.parameter_types)
                {
                    session.push(
                        Constraint::Equal(TypeTerm::Type(parameter), TypeTerm::Rigid(expected)),
                        ConstraintOrigin::ExpectedResult,
                    );
                }
            }
            session.push(
                Constraint::Equal(
                    TypeTerm::Type(return_type),
                    TypeTerm::Rigid(expected.return_type),
                ),
                ConstraintOrigin::ExpectedResult,
            );
        }
        session.push(
            Constraint::CallableShape(shape, TypeTerm::Rigid(expected_type)),
            ConstraintOrigin::ExpectedResult,
        );

        let solution = self.solve_constraints(&session)?;
        let arguments = solution.arguments_for(&session, environment);
        Ok(arguments
            .owner
            .into_iter()
            .chain(arguments.callable)
            .collect())
    }
}
