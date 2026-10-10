use super::{IntegerDivRem, IntegerKind, IntegerOperation, NoGcIntegerOperation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerUnaryOperator {
    UnaryPlus,
    UnaryMinus,
    Inc,
    Dec,
    Inv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerBinaryOperator {
    Add,
    Sub,
    Mul,
    CompareTo,
    Equals,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
    Div,
    Rem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerOperator {
    Unary(IntegerUnaryOperator),
    Binary(IntegerBinaryOperator),
}

/// A complete fixed-arity operation, shared by HIR and bound templates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerExpression<E> {
    Unary {
        kind: IntegerKind,
        operation: IntegerUnaryOperator,
        operand: E,
    },
    Binary {
        kind: IntegerKind,
        operation: IntegerBinaryOperator,
        lhs: E,
        rhs: E,
    },
}

impl<E> IntegerExpression<E> {
    pub const fn kind(&self) -> IntegerKind {
        match self {
            Self::Unary { kind, .. } | Self::Binary { kind, .. } => *kind,
        }
    }

    pub const fn is_managed(&self) -> bool {
        matches!(
            self,
            Self::Binary {
                operation: IntegerBinaryOperator::Div | IntegerBinaryOperator::Rem,
                ..
            }
        )
    }

    /// The declaration/wire descriptor does not own independently mutable operands.
    pub fn descriptor(&self) -> IntegerOperation {
        match self {
            Self::Unary {
                kind, operation, ..
            } => operation.descriptor(*kind),
            Self::Binary {
                kind, operation, ..
            } => operation.descriptor(*kind),
        }
    }

    pub fn operands(&self) -> impl DoubleEndedIterator<Item = &E> {
        match self {
            Self::Unary { operand, .. } => [Some(operand), None],
            Self::Binary { lhs, rhs, .. } => [Some(lhs), Some(rhs)],
        }
        .into_iter()
        .flatten()
    }

    pub fn operands_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut E> {
        match self {
            Self::Unary { operand, .. } => [Some(operand), None],
            Self::Binary { lhs, rhs, .. } => [Some(lhs), Some(rhs)],
        }
        .into_iter()
        .flatten()
    }

    pub fn map<T>(&self, mut map: impl FnMut(&E) -> T) -> IntegerExpression<T> {
        match self {
            Self::Unary {
                kind,
                operation,
                operand,
            } => IntegerExpression::Unary {
                kind: *kind,
                operation: *operation,
                operand: map(operand),
            },
            Self::Binary {
                kind,
                operation,
                lhs,
                rhs,
            } => IntegerExpression::Binary {
                kind: *kind,
                operation: *operation,
                lhs: map(lhs),
                rhs: map(rhs),
            },
        }
    }

    pub fn try_map<T, Error>(
        &self,
        mut map: impl FnMut(&E) -> Result<T, Error>,
    ) -> Result<IntegerExpression<T>, Error> {
        Ok(match self {
            Self::Unary {
                kind,
                operation,
                operand,
            } => IntegerExpression::Unary {
                kind: *kind,
                operation: *operation,
                operand: map(operand)?,
            },
            Self::Binary {
                kind,
                operation,
                lhs,
                rhs,
            } => IntegerExpression::Binary {
                kind: *kind,
                operation: *operation,
                lhs: map(lhs)?,
                rhs: map(rhs)?,
            },
        })
    }
}

impl IntegerOperation {
    pub fn classify(self) -> IntegerOperator {
        use IntegerBinaryOperator as B;
        use IntegerUnaryOperator as U;
        use NoGcIntegerOperation as N;
        match self {
            Self::NoGc { operation, .. } => match operation {
                N::UnaryPlus => IntegerOperator::Unary(U::UnaryPlus),
                N::UnaryMinus => IntegerOperator::Unary(U::UnaryMinus),
                N::Inc => IntegerOperator::Unary(U::Inc),
                N::Dec => IntegerOperator::Unary(U::Dec),
                N::Inv => IntegerOperator::Unary(U::Inv),
                N::Add => IntegerOperator::Binary(B::Add),
                N::Sub => IntegerOperator::Binary(B::Sub),
                N::Mul => IntegerOperator::Binary(B::Mul),
                N::CompareTo => IntegerOperator::Binary(B::CompareTo),
                N::Equals => IntegerOperator::Binary(B::Equals),
                N::And => IntegerOperator::Binary(B::And),
                N::Or => IntegerOperator::Binary(B::Or),
                N::Xor => IntegerOperator::Binary(B::Xor),
                N::Shl => IntegerOperator::Binary(B::Shl),
                N::Shr => IntegerOperator::Binary(B::Shr),
                N::Ushr => IntegerOperator::Binary(B::Ushr),
            },
            Self::Managed {
                operation: IntegerDivRem::Div,
                ..
            } => IntegerOperator::Binary(B::Div),
            Self::Managed {
                operation: IntegerDivRem::Rem,
                ..
            } => IntegerOperator::Binary(B::Rem),
        }
    }
}

impl IntegerUnaryOperator {
    pub fn descriptor(self, kind: IntegerKind) -> IntegerOperation {
        use NoGcIntegerOperation as N;
        let operation = match self {
            Self::UnaryPlus => N::UnaryPlus,
            Self::UnaryMinus => N::UnaryMinus,
            Self::Inc => N::Inc,
            Self::Dec => N::Dec,
            Self::Inv => N::Inv,
        };
        IntegerOperation::NoGc { kind, operation }
    }
}

impl IntegerBinaryOperator {
    pub fn descriptor(self, kind: IntegerKind) -> IntegerOperation {
        use NoGcIntegerOperation as N;
        let operation = match self {
            Self::Div => {
                return IntegerOperation::Managed {
                    kind,
                    operation: IntegerDivRem::Div,
                };
            }
            Self::Rem => {
                return IntegerOperation::Managed {
                    kind,
                    operation: IntegerDivRem::Rem,
                };
            }
            Self::Add => N::Add,
            Self::Sub => N::Sub,
            Self::Mul => N::Mul,
            Self::CompareTo => N::CompareTo,
            Self::Equals => N::Equals,
            Self::And => N::And,
            Self::Or => N::Or,
            Self::Xor => N::Xor,
            Self::Shl => N::Shl,
            Self::Shr => N::Shr,
            Self::Ushr => N::Ushr,
        };
        IntegerOperation::NoGc { kind, operation }
    }
}
