//! Fixed-point inference with unique expressible solutions.

mod materialization;

use scoop_hir as hir;

use super::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape, CallableShapeMismatch,
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceEnvironmentId,
    InferenceSession, InferenceSessionId, InferenceVariableId, RelationKind, TypeTerm,
};
use super::relations::{AtomicConstraint, AtomicConstraintKind, reduce_constraints};
use crate::{Lowerer, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConcreteInferenceArguments {
    pub(crate) owner: Vec<hir::TypeId>,
    pub(crate) callable: Vec<hir::TypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InferenceSolution {
    session: InferenceSessionId,
    bindings: Vec<(InferenceVariableId, hir::TypeId)>,
}

impl InferenceSolution {
    pub(crate) fn type_for(&self, variable: impl Into<InferenceVariableId>) -> hir::TypeId {
        let variable = variable.into();
        assert_eq!(variable.session(), self.session);
        self.bindings
            .iter()
            .find_map(|(candidate, ty)| (*candidate == variable).then_some(*ty))
            .expect("a successful inference solution binds every variable")
    }

    pub(crate) fn arguments_for(
        &self,
        session: &InferenceSession,
        environment: InferenceEnvironmentId,
    ) -> ConcreteInferenceArguments {
        assert_eq!(session.id(), self.session);
        ConcreteInferenceArguments {
            owner: session
                .owner_variables(environment)
                .iter()
                .map(|variable| self.type_for(*variable))
                .collect(),
            callable: session
                .callable_variables(environment)
                .iter()
                .map(|variable| self.type_for(*variable))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct Bound {
    term: TypeTerm,
    origin: ConstraintOrigin,
}

#[derive(Debug, Clone)]
struct VariableState {
    variable: InferenceVariableId,
    exact: Vec<Bound>,
    lower: Vec<Bound>,
    upper: Vec<Bound>,
    kinds: Vec<(hir::TypeParamKind, ConstraintOrigin)>,
    interfaces: Vec<Bound>,
}

impl VariableState {
    fn new(variable: InferenceVariableId) -> Self {
        Self {
            variable,
            exact: Vec::new(),
            lower: Vec::new(),
            upper: Vec::new(),
            kinds: Vec::new(),
            interfaces: Vec::new(),
        }
    }

    fn failure_origin(&self) -> ConstraintOrigin {
        self.exact
            .first()
            .or_else(|| self.lower.first())
            .or_else(|| self.upper.first())
            .or_else(|| self.interfaces.first())
            .map(|bound| bound.origin)
            .or_else(|| self.kinds.first().map(|(_, origin)| *origin))
            .unwrap_or(ConstraintOrigin::Declaration)
    }
}

impl Lowerer {
    pub(crate) fn solve_constraints(
        &mut self,
        session: &InferenceSession,
    ) -> Result<InferenceSolution, ConstraintFailure> {
        let atomic = reduce_constraints(self, session)?;
        let mut states: Vec<_> = session
            .variables()
            .iter()
            .copied()
            .map(VariableState::new)
            .collect();
        let mut checks = Vec::new();
        distribute_constraints(session, atomic, &mut states, &mut checks)?;
        let mut bindings = vec![None; states.len()];

        loop {
            let mut progress = false;
            for state in &states {
                let index = session
                    .variable_index(state.variable)
                    .expect("variable state belongs to its inference session");
                if bindings[index].is_some() {
                    continue;
                }
                if let Some(solution) = self.try_solve_variable(session, &bindings, state)? {
                    bindings[index] = Some(solution);
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }

        if let Some(state) = states.iter().find(|state| {
            let index = session
                .variable_index(state.variable)
                .expect("variable state belongs to its inference session");
            bindings[index].is_none()
        }) {
            return Err(self.no_unique_failure(session, &bindings, state));
        }

        for state in &states {
            let solution = bindings[session
                .variable_index(state.variable)
                .expect("variable state belongs to its inference session")]
            .expect("all inference variables were solved");
            self.validate_variable(session, &bindings, state, solution)?;
        }
        for check in &checks {
            self.validate_check(session, &bindings, check)?;
        }

        Ok(InferenceSolution {
            session: session.id(),
            bindings: session
                .variables()
                .iter()
                .copied()
                .zip(
                    bindings.into_iter().map(|binding| {
                        binding.expect("a successful inference solution is complete")
                    }),
                )
                .collect(),
        })
    }

    fn try_solve_variable(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        state: &VariableState,
    ) -> Result<Option<hir::TypeId>, ConstraintFailure> {
        let exact = self.materialized_bounds(session, bindings, &state.exact)?;
        if let Some(&candidate) = exact.first() {
            for &other in &exact[1..] {
                if !self.types_equal(candidate, other) {
                    return Err(ConstraintFailure {
                        origin: state.failure_origin(),
                        kind: ConstraintFailureKind::ConflictingExactBounds {
                            variable: state.variable,
                            first: candidate,
                            second: other,
                        },
                    });
                }
            }
            return Ok(Some(candidate));
        }

        let lower = self.materialized_bounds(session, bindings, &state.lower)?;
        if lower.len() != state.lower.len() || lower.is_empty() {
            return Ok(None);
        }
        let upper = self.materialized_bounds(session, bindings, &state.upper)?;
        let interfaces = self.materialized_bounds(session, bindings, &state.interfaces)?;
        if upper.len() != state.upper.len() || interfaces.len() != state.interfaces.len() {
            return Ok(None);
        }

        let minimal = self.minimal_solutions(&lower, &upper, &interfaces, &state.kinds);
        Ok((minimal.len() == 1).then_some(minimal[0]))
    }

    fn no_unique_failure(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        state: &VariableState,
    ) -> ConstraintFailure {
        let lower = self
            .materialized_bounds(session, bindings, &state.lower)
            .unwrap_or_default();
        let upper = self
            .materialized_bounds(session, bindings, &state.upper)
            .unwrap_or_default();
        let interfaces = self
            .materialized_bounds(session, bindings, &state.interfaces)
            .unwrap_or_default();
        let minimal = if lower.len() == state.lower.len()
            && upper.len() == state.upper.len()
            && interfaces.len() == state.interfaces.len()
            && !lower.is_empty()
        {
            self.minimal_solutions(&lower, &upper, &interfaces, &state.kinds)
        } else {
            Vec::new()
        };
        ConstraintFailure {
            origin: state.failure_origin(),
            kind: ConstraintFailureKind::NoUniqueSolution {
                variable: state.variable,
                lower_bounds: lower,
                upper_bounds: upper,
                minimal_solutions: minimal,
            },
        }
    }

    fn minimal_solutions(
        &mut self,
        lower: &[hir::TypeId],
        upper: &[hir::TypeId],
        interfaces: &[hir::TypeId],
        kinds: &[(hir::TypeParamKind, ConstraintOrigin)],
    ) -> Vec<hir::TypeId> {
        let mut candidates = Vec::new();
        loop {
            let before = self.types.len();
            let type_ids: Vec<_> = self.types.iter().map(|(id, _)| id).collect();
            for candidate in type_ids {
                if self.type_contains_param(candidate)
                    || candidates
                        .iter()
                        .any(|&existing| self.types_equal(existing, candidate))
                    || !lower.iter().all(|&bound| self.is_subtype(bound, candidate))
                    || !upper.iter().all(|&bound| self.is_subtype(candidate, bound))
                    || !interfaces
                        .iter()
                        .all(|&bound| self.is_subtype(candidate, bound))
                    || !kinds
                        .iter()
                        .all(|&(kind, _)| self.solution_satisfies_kind(candidate, kind))
                {
                    continue;
                }
                candidates.push(candidate);
            }
            if self.types.len() == before {
                break;
            }
        }

        let mut minimal = Vec::new();
        for &candidate in &candidates {
            let mut has_strictly_smaller = false;
            for &other in &candidates {
                if !self.types_equal(other, candidate) && self.is_subtype(other, candidate) {
                    has_strictly_smaller = true;
                    break;
                }
            }
            if !has_strictly_smaller {
                minimal.push(candidate);
            }
        }
        minimal
    }

    fn validate_variable(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        state: &VariableState,
        solution: hir::TypeId,
    ) -> Result<(), ConstraintFailure> {
        for bound in &state.exact {
            let required = self.require_materialized(session, bindings, bound)?;
            if !self.types_equal(solution, required) {
                return Err(ConstraintFailure {
                    origin: bound.origin,
                    kind: ConstraintFailureKind::ConflictingExactBounds {
                        variable: state.variable,
                        first: solution,
                        second: required,
                    },
                });
            }
        }
        for bound in &state.lower {
            let lower = self.require_materialized(session, bindings, bound)?;
            if !self.is_subtype(lower, solution) {
                return Err(relation_failure(
                    bound.origin,
                    RelationKind::Subtype,
                    lower.into(),
                    solution.into(),
                ));
            }
        }
        for bound in &state.upper {
            let upper = self.require_materialized(session, bindings, bound)?;
            if !self.is_subtype(solution, upper) {
                return Err(relation_failure(
                    bound.origin,
                    RelationKind::Subtype,
                    solution.into(),
                    upper.into(),
                ));
            }
        }
        for &(kind, origin) in &state.kinds {
            if !self.solution_satisfies_kind(solution, kind) {
                return Err(ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::Kind {
                        variable: state.variable,
                        solution,
                        required: kind,
                    },
                });
            }
        }
        for bound in &state.interfaces {
            let required = self.require_materialized(session, bindings, bound)?;
            if !self.is_subtype(solution, required) {
                return Err(ConstraintFailure {
                    origin: bound.origin,
                    kind: ConstraintFailureKind::InterfaceBound {
                        variable: state.variable,
                        solution,
                        required,
                    },
                });
            }
        }
        Ok(())
    }

    fn validate_check(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        check: &AtomicConstraint,
    ) -> Result<(), ConstraintFailure> {
        match &check.kind {
            AtomicConstraintKind::CallableShape(shape, expected) => {
                let expected = self
                    .materialize_term(session, bindings, *expected, check.origin)?
                    .ok_or(ConstraintFailure {
                        origin: check.origin,
                        kind: ConstraintFailureKind::CallableShape(
                            CallableShapeMismatch::ExpectedCallable,
                        ),
                    })?;
                self.validate_callable_shape(session, bindings, shape, expected, check.origin)
            }
            AtomicConstraintKind::ConcreteApplication(application) => self
                .materialize_application(session, bindings, application, check.origin)
                .map(|_| ()),
            AtomicConstraintKind::Exact(..)
            | AtomicConstraintKind::LowerBound(..)
            | AtomicConstraintKind::UpperBound(..)
            | AtomicConstraintKind::Kind(..)
            | AtomicConstraintKind::Implements(..) => Ok(()),
        }
    }

    fn validate_callable_shape(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        shape: &CallableShape,
        expected: hir::TypeId,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        let signature = match (shape.category, self.types[expected].clone()) {
            (CallableCategory::Managed, Type::Function(signature))
            | (CallableCategory::Native, Type::FunPtr(signature)) => {
                self.function_types[signature].clone()
            }
            _ => {
                return Err(callable_failure(
                    origin,
                    CallableShapeMismatch::ExpectedCallable,
                ));
            }
        };
        if shape.is_suspend != signature.is_suspend {
            return Err(callable_failure(origin, CallableShapeMismatch::Suspend));
        }
        if shape.parameters.len() != signature.parameter_types.len() {
            return Err(callable_failure(
                origin,
                CallableShapeMismatch::Arity {
                    expected: signature.parameter_types.len(),
                    actual: shape.parameters.len(),
                },
            ));
        }
        for (actual, expected) in shape.parameters.iter().zip(signature.parameter_types) {
            if let CallableParameter::Explicit(actual) = actual {
                let actual = self
                    .materialize_term(session, bindings, *actual, origin)?
                    .ok_or_else(|| {
                        callable_failure(origin, CallableShapeMismatch::ExpectedCallable)
                    })?;
                if !self.is_subtype(expected, actual) {
                    return Err(relation_failure(
                        origin,
                        RelationKind::Subtype,
                        expected.into(),
                        actual.into(),
                    ));
                }
            }
        }
        if let CallableReturn::Explicit(actual) = shape.return_type {
            let actual = self
                .materialize_term(session, bindings, actual, origin)?
                .ok_or_else(|| callable_failure(origin, CallableShapeMismatch::ExpectedCallable))?;
            if !self.is_subtype(actual, signature.return_type) {
                return Err(relation_failure(
                    origin,
                    RelationKind::Subtype,
                    actual.into(),
                    signature.return_type.into(),
                ));
            }
        }
        Ok(())
    }

    fn solution_satisfies_kind(&self, solution: hir::TypeId, kind: hir::TypeParamKind) -> bool {
        match kind {
            hir::TypeParamKind::Any => true,
            hir::TypeParamKind::Value => self.is_value_ty(solution),
            hir::TypeParamKind::Ref => self.is_ref_ty(solution),
        }
    }
}

fn distribute_constraints(
    session: &InferenceSession,
    atomic: Vec<AtomicConstraint>,
    states: &mut [VariableState],
    checks: &mut Vec<AtomicConstraint>,
) -> Result<(), ConstraintFailure> {
    for constraint in atomic {
        let (variable, destination) = match constraint.kind {
            AtomicConstraintKind::Exact(variable, term) => (variable, Some((0, term))),
            AtomicConstraintKind::LowerBound(variable, term) => (variable, Some((1, term))),
            AtomicConstraintKind::UpperBound(variable, term) => (variable, Some((2, term))),
            AtomicConstraintKind::Implements(variable, term) => (variable, Some((3, term))),
            AtomicConstraintKind::Kind(variable, kind) => {
                let index = session.variable_index(variable).ok_or(ConstraintFailure {
                    origin: constraint.origin,
                    kind: ConstraintFailureKind::ForeignVariable(variable),
                })?;
                states[index].kinds.push((kind, constraint.origin));
                continue;
            }
            AtomicConstraintKind::CallableShape(..)
            | AtomicConstraintKind::ConcreteApplication(..) => {
                checks.push(constraint);
                continue;
            }
        };
        let index = session.variable_index(variable).ok_or(ConstraintFailure {
            origin: constraint.origin,
            kind: ConstraintFailureKind::ForeignVariable(variable),
        })?;
        let (destination, term) = destination.expect("variable constraint has a bound");
        let bound = Bound {
            term,
            origin: constraint.origin,
        };
        match destination {
            0 => states[index].exact.push(bound),
            1 => states[index].lower.push(bound),
            2 => states[index].upper.push(bound),
            3 => states[index].interfaces.push(bound),
            _ => unreachable!("closed bound destination"),
        }
    }
    Ok(())
}

fn relation_failure(
    origin: ConstraintOrigin,
    relation: RelationKind,
    left: TypeTerm,
    right: TypeTerm,
) -> ConstraintFailure {
    ConstraintFailure {
        origin,
        kind: ConstraintFailureKind::Relation {
            relation,
            left,
            right,
        },
    }
}

fn callable_failure(
    origin: ConstraintOrigin,
    mismatch: CallableShapeMismatch,
) -> ConstraintFailure {
    ConstraintFailure {
        origin,
        kind: ConstraintFailureKind::CallableShape(mismatch),
    }
}
