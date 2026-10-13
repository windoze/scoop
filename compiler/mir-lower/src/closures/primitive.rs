use scoop_hir as hir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveReference {
    Unary(UnaryReference),
    Binary(BinaryReference),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnaryReference {
    Primitive(hir::PrimitiveUnaryKind),
    Integer {
        kind: hir::IntegerKind,
        operation: hir::IntegerUnaryOperator,
    },
    IntegerConversion(hir::IntegerConversion),
    Float {
        kind: hir::FloatKind,
        operation: hir::FloatUnaryOperator,
    },
    FloatConversion(hir::HirFloatConversion),
    CharCode,
    CharFromCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BinaryReference {
    Primitive(hir::PrimitiveBinaryKind),
    Integer {
        kind: hir::IntegerKind,
        operation: hir::IntegerBinaryOperator,
    },
    Float {
        kind: hir::FloatKind,
        operation: hir::FloatBinaryOperator,
    },
    CharEquals,
    CharCompareTo,
}

impl PrimitiveReference {
    pub(crate) fn from_intrinsic(kind: hir::IntrinsicFunctionKind) -> Option<Self> {
        use hir::{IntrinsicFunctionKind as I, PrimitiveMemberIntrinsic as P};
        Some(Self::classify(match kind {
            I::Integer(kind) => P::Integer(kind),
            I::Float(kind) => P::Float(kind),
            I::Char(kind) => P::Char(kind),
            I::PrimitiveUnary(kind) => P::Unary(kind),
            I::PrimitiveBinary(kind) => P::Binary(kind),
            _ => return None,
        }))
    }
    pub(crate) fn classify(intrinsic: hir::PrimitiveMemberIntrinsic) -> Self {
        use hir::{
            CharIntrinsic as C, FloatIntrinsicKind as F, IntegerIntrinsicKind as I,
            PrimitiveMemberIntrinsic as P,
        };
        match intrinsic {
            P::Unary(kind) => Self::Unary(UnaryReference::Primitive(kind)),
            P::Binary(kind) => Self::Binary(BinaryReference::Primitive(kind)),
            P::Char(C::Code) => Self::Unary(UnaryReference::CharCode),
            P::Char(C::FromCodeUnchecked) => Self::Unary(UnaryReference::CharFromCode),
            P::Char(C::Equals) => Self::Binary(BinaryReference::CharEquals),
            P::Char(C::CompareTo) => Self::Binary(BinaryReference::CharCompareTo),
            P::Float(F::Unary { kind, operation }) => {
                Self::Unary(UnaryReference::Float { kind, operation })
            }
            P::Float(F::Binary { kind, operation }) => {
                Self::Binary(BinaryReference::Float { kind, operation })
            }
            P::Float(F::Conversion(conversion)) => {
                Self::Unary(UnaryReference::FloatConversion(conversion))
            }
            P::Integer(I::Conversion {
                source,
                target_kind,
            }) => Self::Unary(UnaryReference::IntegerConversion(hir::IntegerConversion {
                source,
                target_kind,
            })),
            P::Integer(I::NoGcOperation { kind, operation }) => {
                Self::integer(hir::IntegerOperation::NoGc { kind, operation })
            }
            P::Integer(I::ManagedOperation { kind, operation }) => {
                Self::integer(hir::IntegerOperation::Managed { kind, operation })
            }
        }
    }

    fn integer(descriptor: hir::IntegerOperation) -> Self {
        let kind = descriptor.kind();
        match descriptor.classify() {
            hir::IntegerOperator::Unary(operation) => {
                Self::Unary(UnaryReference::Integer { kind, operation })
            }
            hir::IntegerOperator::Binary(operation) => {
                Self::Binary(BinaryReference::Integer { kind, operation })
            }
        }
    }
}
