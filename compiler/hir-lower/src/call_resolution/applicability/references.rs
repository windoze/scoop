//! Exact managed and native reference signatures share declaration constraints.

use super::*;
use crate::Type;
use crate::call_resolution::candidates::{CallableEffects, DeclarationSignature};
use crate::call_resolution::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape,
};

pub(crate) struct CallableReferenceApplicabilityInput<'a, D> {
    pub(crate) signature: &'a DeclarationSignature<D>,
    pub(crate) owner_arguments: &'a [hir::TypeId],
    pub(crate) bound_receiver: Option<(hir::TypeId, hir::TypeId)>,
    pub(crate) unbound_receiver: Option<hir::TypeId>,
    pub(crate) effects: CallableEffects,
    pub(crate) expected_type: Option<hir::TypeId>,
}

pub(crate) struct ApplicableReferenceSignature {
    pub(crate) type_args: Vec<hir::TypeId>,
    pub(crate) ty: hir::TypeId,
}

pub(crate) enum ReferenceApplicabilityFailure {
    IncompleteOwner,
    RequiresExpected,
    Unsafe,
    Constraint(ConstraintFailure),
}

impl ReferenceApplicabilityFailure {
    pub(crate) fn describe(self, constraint: impl FnOnce(&ConstraintFailure) -> String) -> String {
        match self {
            Self::IncompleteOwner => "receiver does not provide complete owner type arguments".into(),
            Self::RequiresExpected => "generic callable references require an expected function type".into(),
            Self::Unsafe => "unsafe functions cannot be stored in a managed function type because safety is not part of function-type identity".into(),
            Self::Constraint(failure) => constraint(&failure),
        }
    }
}

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

    pub(crate) fn solve_callable_reference_applicability<D>(
        &mut self,
        input: CallableReferenceApplicabilityInput<'_, D>,
    ) -> Result<ApplicableReferenceSignature, ReferenceApplicabilityFailure> {
        let CallableReferenceApplicabilityInput {
            signature,
            owner_arguments,
            bound_receiver,
            unbound_receiver,
            effects,
            expected_type,
        } = input;
        let owner_parameters = &signature.owner_parameters;
        let callable_parameters = &signature.callable_parameters;
        if owner_parameters.len() != owner_arguments.len() {
            return Err(ReferenceApplicabilityFailure::IncompleteOwner);
        }
        if expected_type.is_none() && !callable_parameters.is_empty() {
            return Err(ReferenceApplicabilityFailure::RequiresExpected);
        }
        if effects.attributes.safety == hir::Safety::Unsafe {
            return Err(ReferenceApplicabilityFailure::Unsafe);
        }
        let parameter_types = unbound_receiver
            .into_iter()
            .chain(
                signature
                    .value_parameters
                    .iter()
                    .map(|parameter| parameter.ty),
            )
            .collect::<Vec<_>>();
        let return_type = signature.return_type;
        let expected_type = expected_type.unwrap_or_else(|| {
            let bindings = owner_parameters
                .iter()
                .map(|parameter| parameter.id)
                .zip(owner_arguments.iter().copied())
                .collect::<Vec<_>>();
            let parameters = parameter_types
                .iter()
                .map(|&ty| self.instantiate_method_ty(ty, &bindings))
                .collect();
            let result = self.instantiate_method_ty(return_type, &bindings);
            self.intern_function_type(effects.is_suspend, parameters, result)
        });
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
            is_suspend: effects.is_suspend,
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

        let solution = self
            .solve_constraints(&session)
            .map_err(ReferenceApplicabilityFailure::Constraint)?;
        let arguments = solution.arguments_for(&session, environment);
        Ok(ApplicableReferenceSignature {
            type_args: arguments
                .owner
                .into_iter()
                .chain(arguments.callable)
                .collect(),
            ty: expected_type,
        })
    }
}
