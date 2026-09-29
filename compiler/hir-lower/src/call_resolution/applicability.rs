//! Constraint construction for one callable candidate.

use scoop_hir as hir;

use super::arguments::CandidateArgumentMap;
use super::candidates::{
    CallableView, NominalConstructorSource, NominalConstructorView, ReceiverShape,
};
use super::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape, Constraint,
    ConstraintFailure, ConstraintOrigin, InferenceSession, NominalApplication, TypeTerm,
};
use crate::expr::ResolvedCallTypeArgument;
use crate::{Lowerer, Type};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CallableApplicabilityInput<'a> {
    pub(crate) view: &'a CallableView,
    pub(crate) owner_arguments: &'a [hir::TypeId],
    pub(crate) explicit_arguments: &'a [ResolvedCallTypeArgument],
    pub(crate) receiver_type: Option<hir::TypeId>,
}

#[derive(Debug, Clone)]
pub(crate) struct NominalApplicabilityInput<'a> {
    pub(crate) view: &'a NominalConstructorView,
    pub(crate) argument_map: &'a CandidateArgumentMap,
    pub(crate) explicit_arguments: &'a [ResolvedCallTypeArgument],
    pub(crate) expected_arguments: Option<&'a [hir::TypeId]>,
    pub(crate) argument_types: &'a [Option<hir::TypeId>],
}

#[derive(Debug, Clone)]
pub(crate) struct CallableReferenceApplicabilityInput<'a> {
    pub(crate) owner_parameters: &'a [hir::TypeParamDecl],
    pub(crate) callable_parameters: &'a [hir::TypeParamDecl],
    pub(crate) owner_arguments: &'a [hir::TypeId],
    pub(crate) bound_receiver: Option<(hir::TypeId, hir::TypeId)>,
    pub(crate) parameter_types: &'a [hir::TypeId],
    pub(crate) return_type: hir::TypeId,
    pub(crate) is_suspend: bool,
    pub(crate) expected_type: hir::TypeId,
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
        debug_assert_eq!(owner_parameters.len(), owner_arguments.len());

        let mut session = InferenceSession::new();
        let environment = session.add_environment(owner_parameters, callable_parameters);
        for (&variable, &argument) in session
            .owner_variables(environment)
            .to_vec()
            .iter()
            .zip(owner_arguments)
        {
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                ConstraintOrigin::Receiver,
            );
        }
        self.add_declaration_bounds(
            &mut session,
            owner_parameters.iter().chain(callable_parameters),
        );

        if let Some((declared, actual)) = bound_receiver {
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(actual), TypeTerm::Type(declared)),
                ConstraintOrigin::Receiver,
            );
        }

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

    pub(crate) fn callable_applicability_session(
        &self,
        input: CallableApplicabilityInput<'_>,
    ) -> (InferenceSession, super::constraints::InferenceEnvironmentId) {
        let CallableApplicabilityInput {
            view,
            owner_arguments,
            explicit_arguments,
            receiver_type,
        } = input;
        debug_assert_eq!(view.owner_parameters.len(), owner_arguments.len());
        debug_assert!(
            explicit_arguments.is_empty()
                || view.callable_parameters.len() == explicit_arguments.len()
        );

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
        debug_assert!(
            explicit_arguments.is_empty()
                || explicit_arguments.len() == view.owner_parameters.len()
        );
        debug_assert!(
            expected_arguments
                .is_none_or(|arguments| arguments.len() == view.owner_parameters.len())
        );
        debug_assert_eq!(argument_map.source_order.len(), argument_types.len());
        let mut session = InferenceSession::new();
        let environment = session.add_environment(&view.owner_parameters, &[]);
        let owner_variables = session.owner_variables(environment).to_vec();

        for (index, (&variable, &argument)) in
            owner_variables.iter().zip(explicit_arguments).enumerate()
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
        if let Some(expected_arguments) = expected_arguments {
            for (&variable, &argument) in owner_variables.iter().zip(expected_arguments) {
                session.push(
                    Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                    ConstraintOrigin::ExpectedResult,
                );
            }
        }

        self.add_declaration_bounds(&mut session, view.owner_parameters.iter());
        for (index, (pattern, actual)) in argument_map
            .inference_patterns(&view.value_parameters)
            .into_iter()
            .zip(argument_types)
            .enumerate()
        {
            if let Some(actual) = actual {
                pattern.constrain(&mut session, index, *actual);
            }
        }

        let application_arguments = owner_variables
            .iter()
            .copied()
            .map(TypeTerm::from)
            .collect();
        let application = match view.target {
            NominalConstructorSource::Struct(constructor) => NominalApplication::Struct(
                self.struct_constructors[constructor].owner,
                application_arguments,
            ),
            NominalConstructorSource::Class(constructor) => NominalApplication::Class(
                self.class_constructors[constructor].owner,
                application_arguments,
            ),
            NominalConstructorSource::IntrinsicClass(class) => {
                NominalApplication::Class(class, application_arguments)
            }
            NominalConstructorSource::ImportedArray(owner) => {
                NominalApplication::Imported(owner, application_arguments)
            }
            NominalConstructorSource::Variant(variant) => {
                NominalApplication::Enum(variant.enumeration(), application_arguments)
            }
        };
        session.push(
            Constraint::ConcreteApplication(application),
            ConstraintOrigin::Declaration,
        );

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
