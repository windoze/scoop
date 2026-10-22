use super::*;

leaf!(hir::IntegerUnaryOperator, {UnaryPlus: "unaryPlus", UnaryMinus: "unaryMinus", Inc: "inc", Dec: "dec", Inv: "inv"});
leaf!(hir::IntegerBinaryOperator, {Add: "add", Sub: "sub", Mul: "mul", CompareTo: "compareTo", Equals: "equals", And: "and", Or: "or", Xor: "xor", Shl: "shl", Shr: "shr", Ushr: "ushr", Div: "div", Rem: "rem"});
leaf!(hir::FloatUnaryOperator, {Plus: "plus", Negate: "negate", Increment: "increment", Decrement: "decrement", IsNaN: "isNaN", IsInfinite: "isInfinite", IsFinite: "isFinite"});
leaf!(hir::FloatBinaryOperator, {Add: "add", Subtract: "subtract", Multiply: "multiply", Divide: "divide", Remainder: "remainder", CompareTo: "compareTo", Equal: "equal", NotEqual: "notEqual", Less: "less", LessEqual: "lessEqual", Greater: "greater", GreaterEqual: "greaterEqual"});

impl Serialize for QueryJson<'_, hir::DefaultIntegerKindV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        QueryJson(&hir::IntegerKind::from(*self.0)).serialize(serializer)
    }
}

impl Serialize for QueryJson<'_, hir::FloatKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"kind": "float", "width": self.0.bits()})
    }
}

impl Serialize for QueryJson<'_, hir::DefaultFloatConversionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            identity::FloatConversion::FromInteger { source, target } => {
                object!(serializer, {"kind": "fromInteger", "source": QueryJson(source), "target": QueryJson(target)})
            }
            identity::FloatConversion::ToInteger { source, target } => {
                object!(serializer, {"kind": "toInteger", "source": QueryJson(source), "target": QueryJson(target)})
            }
            identity::FloatConversion::BetweenFloats { source, target } => {
                object!(serializer, {"kind": "betweenFloats", "source": QueryJson(source), "target": QueryJson(target)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultIntegerExpressionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::IntegerExpression::Unary {
                kind,
                operation,
                operand,
            } => {
                object!(serializer, {"kind": "unary", "integerKind": QueryJson(kind), "operator": QueryJson(operation), "operand": self.child(operand)})
            }
            hir::IntegerExpression::Binary {
                kind,
                operation,
                lhs,
                rhs,
            } => {
                object!(serializer, {"kind": "binary", "integerKind": QueryJson(kind), "operator": QueryJson(operation), "lhs": self.child(lhs), "rhs": self.child(rhs)})
            }
        }
    }
}
