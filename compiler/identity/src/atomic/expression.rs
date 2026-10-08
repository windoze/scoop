use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicExpression<T> {
    pub object: T,
    pub kind: AtomicValueKind,
    pub operation: AtomicOperation<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicOperation<T> {
    Load {
        order: AtomicLoadOrder,
    },
    Store {
        value: T,
        order: AtomicStoreOrder,
    },
    Rmw {
        value: T,
        operation: AtomicRmwOperation,
        order: AtomicMemoryOrder,
    },
    CompareExchange {
        expected: T,
        value: T,
        order: AtomicCompareExchangeOrder,
        result: AtomicCompareExchangeResult,
    },
}

impl<T> AtomicExpression<T> {
    pub fn operands(&self) -> impl DoubleEndedIterator<Item = &T> {
        let operands = match &self.operation {
            AtomicOperation::Load { .. } => [None, None],
            AtomicOperation::Store { value, .. } | AtomicOperation::Rmw { value, .. } => {
                [Some(value), None]
            }
            AtomicOperation::CompareExchange {
                expected, value, ..
            } => [Some(expected), Some(value)],
        };
        std::iter::once(&self.object).chain(operands.into_iter().flatten())
    }

    pub fn operands_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        let operands = match &mut self.operation {
            AtomicOperation::Load { .. } => [None, None],
            AtomicOperation::Store { value, .. } | AtomicOperation::Rmw { value, .. } => {
                [Some(value), None]
            }
            AtomicOperation::CompareExchange {
                expected, value, ..
            } => [Some(expected), Some(value)],
        };
        std::iter::once(&mut self.object).chain(operands.into_iter().flatten())
    }

    pub fn map<'a, U>(&'a self, mut map: impl FnMut(&'a T) -> U) -> AtomicExpression<U> {
        match self.try_map(|value| Ok::<_, std::convert::Infallible>(map(value))) {
            Ok(expression) => expression,
            Err(never) => match never {},
        }
    }

    pub fn try_map<'a, U, E>(
        &'a self,
        mut map: impl FnMut(&'a T) -> Result<U, E>,
    ) -> Result<AtomicExpression<U>, E> {
        let object = map(&self.object)?;
        let operation = match &self.operation {
            AtomicOperation::Load { order } => AtomicOperation::Load { order: *order },
            AtomicOperation::Store { value, order } => AtomicOperation::Store {
                value: map(value)?,
                order: *order,
            },
            AtomicOperation::Rmw {
                value,
                operation,
                order,
            } => AtomicOperation::Rmw {
                value: map(value)?,
                operation: *operation,
                order: *order,
            },
            AtomicOperation::CompareExchange {
                expected,
                value,
                order,
                result,
            } => AtomicOperation::CompareExchange {
                expected: map(expected)?,
                value: map(value)?,
                order: *order,
                result: *result,
            },
        };
        Ok(AtomicExpression {
            object,
            kind: self.kind,
            operation,
        })
    }

    pub fn try_into_map<U, E>(
        self,
        mut map: impl FnMut(T) -> Result<U, E>,
    ) -> Result<AtomicExpression<U>, E> {
        let object = map(self.object)?;
        let operation = match self.operation {
            AtomicOperation::Load { order } => AtomicOperation::Load { order },
            AtomicOperation::Store { value, order } => AtomicOperation::Store {
                value: map(value)?,
                order,
            },
            AtomicOperation::Rmw {
                value,
                operation,
                order,
            } => AtomicOperation::Rmw {
                value: map(value)?,
                operation,
                order,
            },
            AtomicOperation::CompareExchange {
                expected,
                value,
                order,
                result,
            } => AtomicOperation::CompareExchange {
                expected: map(expected)?,
                value: map(value)?,
                order,
                result,
            },
        };
        Ok(AtomicExpression {
            object,
            kind: self.kind,
            operation,
        })
    }
}

impl<T> std::fmt::Display for AtomicOperation<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load { order } => write!(f, "AtomicLoad {order:?}"),
            Self::Store { order, .. } => write!(f, "AtomicStore {order:?}"),
            Self::Rmw {
                operation, order, ..
            } => write!(f, "AtomicRmw {operation:?} {order:?}"),
            Self::CompareExchange { order, result, .. } => {
                write!(f, "AtomicCmpXchg {order:?} {result:?}")
            }
        }
    }
}
