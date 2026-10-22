//! Check completed assignments using the original constraint diagnostics.

use super::{
    AtomicConstraint, AtomicConstraintKind, ConstraintFailure, ConstraintFailureKind,
    ConstraintOrigin, InferenceSession, NominalBoundKind, TypeTerm, VariableState, hir,
};
use crate::call_resolution::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape, CallableShapeMismatch,
    RelationKind,
};
use crate::{Lowerer, Type};

impl Lowerer {
    pub(super) fn validate_fixed_variable_relations(
        &mut self,
        session: &InferenceSession,
        state: &VariableState,
    ) -> Result<(), ConstraintFailure> {
        let Some(fixed) = state
            .exact
            .iter()
            .find_map(|bound| rigid_type(self, session, bound.term))
        else {
            return Ok(());
        };
        for (bounds, lower) in [(&state.lower, true), (&state.upper, false)] {
            for bound in bounds {
                let Some(ty) = rigid_type(self, session, bound.term) else {
                    continue;
                };
                let (left, right) = if lower { (ty, fixed) } else { (fixed, ty) };
                if !self.is_subtype(left, right) {
                    return Err(relation_failure(
                        bound.origin,
                        RelationKind::Subtype,
                        left.into(),
                        right.into(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_variable(
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
            if !self.type_satisfies_kind(solution, kind) {
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
        for nominal in &state.nominal {
            let required = self.require_materialized(session, bindings, &nominal.bound)?;
            if !self.is_subtype(solution, required) {
                let kind = match nominal.kind {
                    NominalBoundKind::Class => ConstraintFailureKind::ClassBound {
                        variable: state.variable,
                        solution,
                        required,
                    },
                    NominalBoundKind::Interface => ConstraintFailureKind::InterfaceBound {
                        variable: state.variable,
                        solution,
                        required,
                    },
                };
                return Err(ConstraintFailure {
                    origin: nominal.bound.origin,
                    kind,
                });
            }
        }
        Ok(())
    }

    pub(super) fn validate_check(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        check: &AtomicConstraint,
    ) -> Result<(), ConstraintFailure> {
        match &check.kind {
            AtomicConstraintKind::MemberRequirement(requirement) => {
                let key = requirement.contract.key(|ty| {
                    let term = TypeTerm::Type(ty);
                    self.materialize_term(session, bindings, term, check.origin)?
                        .ok_or(ConstraintFailure {
                            origin: check.origin,
                            kind: ConstraintFailureKind::UnresolvedTerm(term),
                        })
                })?;
                self.check_member_requirement(
                    requirement.id,
                    &hir::MemberRequirementContract::from_key(key),
                )
                .map(|_| ())
                .map_err(|message| ConstraintFailure {
                    origin: check.origin,
                    kind: ConstraintFailureKind::MemberRequirement(message),
                })
            }
            AtomicConstraintKind::Companion(variable, bounds) => self.validate_companion_condition(
                session,
                bindings,
                *variable,
                bounds,
                check.origin,
            ),
            AtomicConstraintKind::DeferredRelation(relation, left, right) => {
                let left_ty = self.require_materialized(
                    session,
                    bindings,
                    &super::Bound {
                        term: *left,
                        origin: check.origin,
                    },
                )?;
                let right_ty = self.require_materialized(
                    session,
                    bindings,
                    &super::Bound {
                        term: *right,
                        origin: check.origin,
                    },
                )?;
                let satisfied = match relation {
                    RelationKind::Equal => self.types_equal(left_ty, right_ty),
                    RelationKind::Subtype => self.is_subtype(left_ty, right_ty),
                };
                if satisfied {
                    Ok(())
                } else {
                    Err(relation_failure(check.origin, *relation, *left, *right))
                }
            }
            AtomicConstraintKind::ExcludesInterface(variable, interface) => {
                let subject = TypeTerm::Variable(*variable);
                let solution = self
                    .materialize_term(session, bindings, subject, check.origin)?
                    .ok_or(ConstraintFailure {
                        origin: check.origin,
                        kind: ConstraintFailureKind::UnresolvedTerm(subject),
                    })?;
                let required = self
                    .materialize_term(session, bindings, *interface, check.origin)?
                    .ok_or(ConstraintFailure {
                        origin: check.origin,
                        kind: ConstraintFailureKind::UnresolvedTerm(*interface),
                    })?;
                let outcome = self.excludes_interface(solution, required);
                if outcome == crate::types::InterfaceExclusion::Satisfied {
                    Ok(())
                } else {
                    Err(ConstraintFailure {
                        origin: check.origin,
                        kind: ConstraintFailureKind::ExcludedInterface {
                            variable: *variable,
                            solution,
                            required,
                            outcome,
                        },
                    })
                }
            }
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
            | AtomicConstraintKind::ClassBound(..)
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
        if shape.parameters.len() != signature.arity() {
            return Err(callable_failure(
                origin,
                CallableShapeMismatch::Arity {
                    expected: signature.arity(),
                    actual: shape.parameters.len(),
                },
            ));
        }
        for (actual, expected) in shape.parameters.iter().zip(signature.parameters().copied()) {
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
}

fn rigid_type(
    lowerer: &Lowerer,
    session: &InferenceSession,
    term: TypeTerm,
) -> Option<hir::TypeId> {
    match term {
        TypeTerm::Rigid(ty) => Some(ty),
        TypeTerm::Type(ty) if !super::type_contains_session_parameter(lowerer, session, ty) => {
            Some(ty)
        }
        TypeTerm::Type(_) | TypeTerm::Variable(_) => None,
    }
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
