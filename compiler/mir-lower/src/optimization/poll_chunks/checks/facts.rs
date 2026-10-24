//! Values at the current point, so a later assignment never rewrites a snapshot.

use std::collections::{BTreeMap, BTreeSet};

use super::super::{
    header::Header,
    mir,
    values::{Length, defined_local, definition, integer},
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::optimization::poll_chunks) enum Value {
    Unknown,
    Index,
    Bound,
    Integer(i128),
    Boolean(bool),
    Array(Length),
    Length(Length),
}

pub(in crate::optimization::poll_chunks) struct State<'a> {
    head: &'a Header,
    values: BTreeMap<mir::LocalId, Value>,
    pub guards: BTreeSet<Length>,
}

impl<'a> State<'a> {
    pub fn new(head: &'a Header) -> Self {
        Self {
            head,
            values: BTreeMap::from([(head.index, Value::Index)]),
            guards: BTreeSet::new(),
        }
    }

    pub fn statement(&mut self, statement: &mir::Statement) {
        if let Some((local, expression)) = definition(statement) {
            let value = self.expression(expression);
            self.values.insert(local, value);
        } else if let Some(local) = defined_local(statement) {
            self.values.insert(local, Value::Unknown);
        }
    }

    pub fn bind(&mut self, callee: &mir::Function, arguments: &[mir::Expr]) -> Self {
        // Every callee local shadows caller ids, including unknown values.
        // Length identities remain analysis-only and never enter callee IR.
        let mut values = callee
            .body
            .locals
            .iter()
            .map(|(local, _)| (local, Value::Unknown))
            .collect::<BTreeMap<_, _>>();
        for (parameter, argument) in callee.params.iter().zip(arguments) {
            values.insert(parameter.local, self.expression(argument));
        }
        Self {
            head: self.head,
            values,
            guards: self.guards.clone(),
        }
    }

    pub fn expression(&mut self, expression: &mir::Expr) -> Value {
        match &expression.kind {
            mir::ExprKind::Local(local) => {
                if let Some(value) = self.values.get(local) {
                    return *value;
                }
                if let Some(value) = integer(self.head.invariants.resolve(expression)) {
                    return Value::Integer(value);
                }
                if let Some(array) = self.head.invariants.length(expression) {
                    return Value::Length(array);
                }
                if let Some(array) = self.head.invariants.array(expression) {
                    return Value::Array(array);
                }
                if matches!(self.head.bound.kind, mir::ExprKind::Local(bound) if bound == *local) {
                    return Value::Bound;
                }
                Value::Unknown
            }
            mir::ExprKind::IntegerLiteral(_) => Value::Integer(integer(expression).unwrap()),
            mir::ExprKind::BoolLiteral(value) => Value::Boolean(*value),
            mir::ExprKind::ArrayLen { operand, .. } => match self.expression(operand) {
                Value::Array(array) => Value::Length(array),
                _ => Value::Unknown,
            },
            mir::ExprKind::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => {
                let lhs = self.expression(lhs);
                let rhs = self.expression(rhs);
                self.compare(operation.operator(), lhs, rhs)
            }
            mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                operand,
            } => match self.expression(operand) {
                Value::Boolean(value) => Value::Boolean(!value),
                _ => Value::Unknown,
            },
            _ => Value::Unknown,
        }
    }

    fn compare(
        &mut self,
        operator: mir::IntegerComparisonOperator,
        lhs: Value,
        rhs: Value,
    ) -> Value {
        use mir::IntegerComparisonOperator as Op;
        let ordering = match (lhs, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => Some(lhs.cmp(&rhs)),
            (Value::Index, Value::Index) => Some(std::cmp::Ordering::Equal),
            (Value::Index, Value::Bound) => Some(std::cmp::Ordering::Less),
            (Value::Bound, Value::Index) => Some(std::cmp::Ordering::Greater),
            (Value::Index, Value::Length(array)) => {
                self.guard(array);
                Some(std::cmp::Ordering::Less)
            }
            (Value::Length(array), Value::Index) => {
                self.guard(array);
                Some(std::cmp::Ordering::Greater)
            }
            (Value::Index, Value::Integer(value)) if value < 0 => Some(std::cmp::Ordering::Greater),
            (Value::Integer(value), Value::Index) if value < 0 => Some(std::cmp::Ordering::Less),
            (Value::Index, Value::Integer(0)) => {
                return match operator {
                    Op::LessThan => Value::Boolean(false),
                    Op::GreaterThanOrEqual => Value::Boolean(true),
                    _ => Value::Unknown,
                };
            }
            (Value::Integer(0), Value::Index) => {
                return match operator {
                    Op::GreaterThan => Value::Boolean(false),
                    Op::LessThanOrEqual => Value::Boolean(true),
                    _ => Value::Unknown,
                };
            }
            _ => None,
        };
        let Some(ordering) = ordering else {
            return Value::Unknown;
        };
        Value::Boolean(match operator {
            Op::Equal => ordering.is_eq(),
            Op::NotEqual => !ordering.is_eq(),
            Op::LessThan => ordering.is_lt(),
            Op::LessThanOrEqual => !ordering.is_gt(),
            Op::GreaterThan => ordering.is_gt(),
            Op::GreaterThanOrEqual => !ordering.is_lt(),
        })
    }

    fn guard(&mut self, array: Length) {
        if self.head.invariants.length(&self.head.bound) != Some(array) {
            self.guards.insert(array);
        }
    }
}
