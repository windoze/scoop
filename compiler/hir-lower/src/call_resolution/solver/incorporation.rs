//! Reduce existing lower/upper relations before choosing inference solutions.

use super::super::relations::{
    AtomicConstraint, AtomicConstraintKind, reduce_relation, type_contains_session_parameter,
};
use super::TypeTerm;
use super::{ConstraintFailure, ConstraintOrigin, InferenceSession, InferenceVariableId};
use crate::call_resolution::constraints::RelationKind;
use crate::{Lowerer, Type};

#[derive(Clone, Copy)]
enum Direction {
    Exact,
    Lower,
    Upper,
    Class,
    Interface,
}

#[derive(Clone, Copy)]
struct VariableBound {
    variable: InferenceVariableId,
    direction: Direction,
    term: TypeTerm,
    origin: ConstraintOrigin,
}

pub(super) fn incorporate_bounds(
    lowerer: &mut Lowerer,
    session: &InferenceSession,
    mut constraints: Vec<AtomicConstraint>,
) -> Result<Vec<AtomicConstraint>, ConstraintFailure> {
    let mut bounds = Vec::new();
    let mut cursor = 0;
    while cursor < constraints.len() {
        for bound in variable_bounds(lowerer, session, &constraints[cursor]) {
            for &previous in &bounds {
                for derived in incorporate_pair(lowerer, session, previous, bound)? {
                    if !constraints.iter().any(|known| known.kind == derived.kind) {
                        constraints.push(derived);
                    }
                }
            }
            bounds.push(bound);
        }
        cursor += 1;
    }
    Ok(constraints)
}

fn variable_bounds(
    lowerer: &Lowerer,
    session: &InferenceSession,
    constraint: &AtomicConstraint,
) -> Vec<VariableBound> {
    let (variable, direction, term) = match constraint.kind {
        AtomicConstraintKind::Exact(variable, term) => (variable, Direction::Exact, term),
        AtomicConstraintKind::LowerBound(variable, term) => (variable, Direction::Lower, term),
        AtomicConstraintKind::UpperBound(variable, term) => (variable, Direction::Upper, term),
        AtomicConstraintKind::ClassBound(variable, term) => (variable, Direction::Class, term),
        AtomicConstraintKind::Implements(variable, term) => (variable, Direction::Interface, term),
        AtomicConstraintKind::Kind(..)
        | AtomicConstraintKind::CallableShape(..)
        | AtomicConstraintKind::ConcreteApplication(..) => return Vec::new(),
    };
    let mut bounds = vec![VariableBound {
        variable,
        direction,
        term,
        origin: constraint.origin,
    }];
    let other = match term {
        TypeTerm::Variable(variable) => Some(variable),
        TypeTerm::Type(ty) => match lowerer.types[ty] {
            Type::Param(parameter) => session.variable_for(parameter),
            _ => None,
        },
        TypeTerm::Rigid(_) => None,
    };
    // A variable-to-variable relation participates at both ends. This view
    // does not add a reciprocal dependency to the materialization solver.
    if let Some(other) = other {
        let reverse = match direction {
            Direction::Exact => Direction::Exact,
            Direction::Lower => Direction::Upper,
            Direction::Upper => Direction::Lower,
            Direction::Class | Direction::Interface => return bounds,
        };
        bounds.push(VariableBound {
            variable: other,
            direction: reverse,
            term: TypeTerm::Variable(variable),
            origin: constraint.origin,
        });
    }
    bounds
}

fn incorporate_pair(
    lowerer: &mut Lowerer,
    session: &InferenceSession,
    left: VariableBound,
    right: VariableBound,
) -> Result<Vec<AtomicConstraint>, ConstraintFailure> {
    use Direction::{Class, Exact, Interface, Lower, Upper};

    if left.variable != right.variable {
        return Ok(Vec::new());
    }
    let (relation, lower, upper) = match (left.direction, right.direction) {
        (Exact, Exact) => (RelationKind::Equal, left, right),
        (Exact | Lower, Upper | Class | Interface) | (Lower, Exact) => {
            (RelationKind::Subtype, left, right)
        }
        (Upper | Class | Interface, Exact | Lower) | (Exact, Lower) => {
            (RelationKind::Subtype, right, left)
        }
        (Lower, Lower) | (Upper | Class | Interface, Upper | Class | Interface) => {
            return Ok(Vec::new());
        }
    };
    if !contains_variable(lowerer, session, lower.term)
        && !contains_variable(lowerer, session, upper.term)
    {
        // Complete facts remain with the existing bound diagnostics.
        return Ok(Vec::new());
    }
    let mut derived = reduce_relation(
        lowerer,
        session,
        relation,
        lower.term,
        upper.term,
        upper.origin,
    )?;
    for constraint in &mut derived {
        if let AtomicConstraintKind::UpperBound(variable, term) = constraint.kind {
            // Forward a declaration bound without making it an inference
            // default for another unconstrained variable.
            constraint.kind = match upper.direction {
                Class => AtomicConstraintKind::ClassBound(variable, term),
                Interface => AtomicConstraintKind::Implements(variable, term),
                Exact | Lower | Upper => constraint.kind.clone(),
            };
        }
    }
    Ok(derived)
}

fn contains_variable(lowerer: &Lowerer, session: &InferenceSession, term: TypeTerm) -> bool {
    match term {
        TypeTerm::Variable(_) => true,
        TypeTerm::Type(ty) => type_contains_session_parameter(lowerer, session, ty),
        TypeTerm::Rigid(_) => false,
    }
}
