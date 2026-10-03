//! Fixed-point inference with unique expressible solutions.

mod incorporation;
mod materialization;
mod validation;

use scoop_hir as hir;

use super::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceEnvironmentId,
    InferenceSession, InferenceSessionId, InferenceVariableId, TypeTerm,
};
use super::relations::{
    AtomicConstraint, AtomicConstraintKind, reduce_constraints, type_contains_session_parameter,
};
use crate::Lowerer;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialInferenceArguments {
    pub(crate) owner: Vec<Option<hir::TypeId>>,
    pub(crate) callable: Vec<Option<hir::TypeId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionMode {
    Complete,
    Satisfiable,
    Partial,
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

#[derive(Debug, Clone, Copy)]
enum NominalBoundKind {
    Class,
    Interface,
}

#[derive(Debug, Clone)]
struct NominalBound {
    kind: NominalBoundKind,
    bound: Bound,
}

#[derive(Debug, Clone)]
struct VariableState {
    variable: InferenceVariableId,
    exact: Vec<Bound>,
    lower: Vec<Bound>,
    upper: Vec<Bound>,
    kinds: Vec<(hir::TypeParamKind, ConstraintOrigin)>,
    nominal: Vec<NominalBound>,
}

impl VariableState {
    fn new(variable: InferenceVariableId) -> Self {
        Self {
            variable,
            exact: Vec::new(),
            lower: Vec::new(),
            upper: Vec::new(),
            kinds: Vec::new(),
            nominal: Vec::new(),
        }
    }

    fn failure_origin(&self) -> ConstraintOrigin {
        self.exact
            .last()
            .or_else(|| self.lower.last())
            .or_else(|| self.upper.last())
            .map(|bound| bound.origin)
            .or_else(|| self.nominal.last().map(|bound| bound.bound.origin))
            .or_else(|| self.kinds.last().map(|(_, origin)| *origin))
            .unwrap_or(ConstraintOrigin::Declaration)
    }
}

impl Lowerer {
    pub(crate) fn solve_constraints(
        &mut self,
        session: &InferenceSession,
    ) -> Result<InferenceSolution, ConstraintFailure> {
        let bindings = self.solve_constraint_bindings(session, CompletionMode::Complete)?;
        Ok(InferenceSolution {
            session: session.id(),
            bindings: session
                .variables()
                .iter()
                .copied()
                .zip(bindings.into_iter().map(|binding| {
                    binding.expect("a complete inference solution binds every variable")
                }))
                .collect(),
        })
    }

    /// MSC asks whether a forwarding relation is satisfiable, not for a
    /// concrete call application. Declaration variables absent from every
    /// forwarding parameter may therefore remain existentially unconstrained.
    pub(crate) fn constraints_are_satisfiable(&mut self, session: &InferenceSession) -> bool {
        self.solve_constraint_bindings(session, CompletionMode::Satisfiable)
            .is_ok()
    }

    /// Propagate every currently decidable relation without requiring the
    /// candidate to be complete yet. Postponed arguments consume these
    /// bindings as expected-type hints and feed their types into the next
    /// fixed-point round.
    pub(crate) fn solve_constraints_partially(
        &mut self,
        session: &InferenceSession,
        environment: InferenceEnvironmentId,
    ) -> Result<PartialInferenceArguments, ConstraintFailure> {
        let bindings = self.solve_constraint_bindings(session, CompletionMode::Partial)?;
        let lookup = |variable: InferenceVariableId| {
            bindings[session
                .variable_index(variable)
                .expect("partial solution variable belongs to its session")]
        };
        Ok(PartialInferenceArguments {
            owner: session
                .owner_variables(environment)
                .iter()
                .copied()
                .map(InferenceVariableId::from)
                .map(lookup)
                .collect(),
            callable: session
                .callable_variables(environment)
                .iter()
                .copied()
                .map(InferenceVariableId::from)
                .map(lookup)
                .collect(),
        })
    }

    fn solve_constraint_bindings(
        &mut self,
        session: &InferenceSession,
        completion: CompletionMode,
    ) -> Result<Vec<Option<hir::TypeId>>, ConstraintFailure> {
        let atomic = reduce_constraints(self, session)?;
        let atomic = incorporation::incorporate_bounds(self, session, atomic)?;
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
                && match completion {
                    CompletionMode::Complete => true,
                    CompletionMode::Satisfiable => {
                        !state.exact.is_empty()
                            || !state.lower.is_empty()
                            || !state.upper.is_empty()
                    }
                    CompletionMode::Partial => false,
                }
        }) {
            return Err(self.no_unique_failure(session, &bindings, state));
        }

        for state in &states {
            let Some(solution) = bindings[session
                .variable_index(state.variable)
                .expect("variable state belongs to its inference session")]
            else {
                debug_assert_ne!(completion, CompletionMode::Complete);
                continue;
            };
            if completion != CompletionMode::Partial {
                self.validate_variable(session, &bindings, state, solution)?;
            }
        }
        if completion != CompletionMode::Partial {
            for check in &checks {
                self.validate_check(session, &bindings, check)?;
            }
        }

        Ok(bindings)
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
        if lower.len() != state.lower.len() {
            return Ok(None);
        }
        let upper = self.materialized_bounds(session, bindings, &state.upper)?;
        if upper.len() != state.upper.len() {
            return Ok(None);
        }
        if lower.is_empty() && upper.is_empty() {
            return Ok(None);
        }

        // Infer the unique representable relation frontier before checking
        // declaration bounds. A value argument must not silently widen to
        // boxed `Any` merely to satisfy `T : ref`; kind and interface bounds
        // validate the inferred concrete application below.
        let frontier = self.solution_frontier(session, &lower, &upper);
        Ok((frontier.len() == 1).then(|| frontier[0]))
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
        let solution_frontier = if lower.len() == state.lower.len()
            && upper.len() == state.upper.len()
            && (!lower.is_empty() || !upper.is_empty())
        {
            self.solution_frontier(session, &lower, &upper)
        } else {
            Vec::new()
        };
        ConstraintFailure {
            origin: state.failure_origin(),
            kind: ConstraintFailureKind::NoUniqueSolution {
                variable: state.variable,
                lower_bounds: lower,
                upper_bounds: upper,
                solution_frontier,
            },
        }
    }

    fn solution_frontier(
        &mut self,
        session: &InferenceSession,
        lower: &[hir::TypeId],
        upper: &[hir::TypeId],
    ) -> Vec<hir::TypeId> {
        let mut candidates = Vec::new();
        loop {
            let before = self.types.len();
            let type_ids: Vec<_> = self.types.iter().map(|(id, _)| id).collect();
            for candidate in type_ids {
                if type_contains_session_parameter(self, session, candidate)
                    || candidates
                        .iter()
                        .any(|&existing| self.types_equal(existing, candidate))
                    || !lower.iter().all(|&bound| self.is_subtype(bound, candidate))
                    || !upper.iter().all(|&bound| self.is_subtype(candidate, bound))
                {
                    continue;
                }
                candidates.push(candidate);
            }
            if self.types.len() == before {
                break;
            }
        }

        // Lower bounds request their unique least common supertype. With only
        // upper bounds, inference is dual: select the unique greatest type
        // below every bound. This lets contravariant positions fix a variable
        // (for example `Continuation<T>`) without inventing a bottom type.
        let prefer_minimal = !lower.is_empty();
        let mut frontier = Vec::new();
        for &candidate in &candidates {
            let mut dominated = false;
            for &other in &candidates {
                let is_better = if prefer_minimal {
                    self.is_subtype(other, candidate)
                } else {
                    self.is_subtype(candidate, other)
                };
                if !self.types_equal(other, candidate) && is_better {
                    dominated = true;
                    break;
                }
            }
            if !dominated {
                frontier.push(candidate);
            }
        }
        frontier
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
            AtomicConstraintKind::ClassBound(variable, term) => (variable, Some((3, term))),
            AtomicConstraintKind::Implements(variable, term) => (variable, Some((4, term))),
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
            3 => states[index].nominal.push(NominalBound {
                kind: NominalBoundKind::Class,
                bound,
            }),
            4 => states[index].nominal.push(NominalBound {
                kind: NominalBoundKind::Interface,
                bound,
            }),
            _ => unreachable!("closed bound destination"),
        }
    }
    Ok(())
}
