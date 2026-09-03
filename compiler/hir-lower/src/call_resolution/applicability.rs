//! Constraint construction for one callable candidate.

use scoop_hir as hir;

use super::arguments::{CandidateArgumentMap, SourceInputId};
use super::candidates::{
    CallableView, NominalConstructorSource, NominalConstructorView, ReceiverShape,
};
use super::constraints::{
    Constraint, ConstraintFailure, ConstraintOrigin, InferenceSession, NominalApplication, TypeTerm,
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

#[derive(Debug, Clone)]
pub(crate) struct NominalApplicabilityInput<'a> {
    pub(crate) view: &'a NominalConstructorView,
    pub(crate) argument_map: &'a CandidateArgumentMap,
    pub(crate) explicit_arguments: &'a [hir::TypeId],
    pub(crate) expected_arguments: Option<&'a [hir::TypeId]>,
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

    pub(crate) fn solve_nominal_applicability(
        &mut self,
        input: NominalApplicabilityInput<'_>,
    ) -> Result<Vec<hir::TypeId>, ConstraintFailure> {
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
        debug_assert_eq!(argument_map.parameters.len(), argument_types.len());
        let (declaration_span, result_type) = match view.target {
            NominalConstructorSource::Struct(structure) => {
                let declaration = &self.structs[structure];
                (
                    declaration.span,
                    self.struct_applications[declaration.self_application].canonical_type,
                )
            }
            NominalConstructorSource::Class(class) => {
                let declaration = &self.classes[class];
                (
                    declaration.span,
                    self.class_applications[declaration.self_application].canonical_type,
                )
            }
            NominalConstructorSource::Variant { enumeration, .. } => {
                let declaration = &self.enums[enumeration];
                (
                    declaration.span,
                    self.enum_applications[declaration.self_application].canonical_type,
                )
            }
        };
        debug_assert_eq!(view.declaration_span, declaration_span);
        debug_assert_eq!(view.result_type, result_type);

        let mut session = InferenceSession::new();
        let environment = session.add_environment(&view.owner_parameters, &[]);
        let owner_variables = session.owner_variables(environment).to_vec();

        for (index, (&variable, &argument)) in
            owner_variables.iter().zip(explicit_arguments).enumerate()
        {
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                ConstraintOrigin::ExplicitTypeArgument(
                    u32::try_from(index).expect("explicit type argument index exceeds u32"),
                ),
            );
        }
        if explicit_arguments.is_empty()
            && let Some(expected_arguments) = expected_arguments
        {
            for (&variable, &argument) in owner_variables.iter().zip(expected_arguments) {
                session.push(
                    Constraint::Equal(variable.into(), TypeTerm::Rigid(argument)),
                    ConstraintOrigin::ExpectedResult,
                );
            }
        }

        self.add_declaration_bounds(&mut session, view.owner_parameters.iter());
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

        let application_arguments = owner_variables
            .iter()
            .copied()
            .map(TypeTerm::from)
            .collect();
        let application = match view.target {
            NominalConstructorSource::Struct(structure) => {
                NominalApplication::Struct(structure, application_arguments)
            }
            NominalConstructorSource::Class(class) => {
                NominalApplication::Class(class, application_arguments)
            }
            NominalConstructorSource::Variant { enumeration, .. } => {
                NominalApplication::Enum(enumeration, application_arguments)
            }
        };
        session.push(
            Constraint::ConcreteApplication(application),
            ConstraintOrigin::Declaration,
        );

        let solution = self.solve_constraints(&session)?;
        Ok(solution.arguments_for(&session, environment).owner)
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
