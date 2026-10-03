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
