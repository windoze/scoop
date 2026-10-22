use super::*;

leaf!(identity::AtomicValueKind, {Int: "int", Long: "long", Boolean: "boolean", Reference: "reference"});
leaf!(identity::AtomicMemoryOrder, {Relaxed: "relaxed", Acquire: "acquire", Release: "release", AcqRel: "acqRel", SeqCst: "seqCst"});
leaf!(identity::AtomicRmwOperation, {Exchange: "exchange", Add: "add", Subtract: "subtract", And: "and", Or: "or", Xor: "xor"});
leaf!(identity::AtomicCompareExchangeResult, {Success: "success", ObservedValue: "observedValue"});

impl Serialize for BodyJson<'_, hir::AtomicExpression<hir::DefaultExpressionV1>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"object": self.child(&self.node.object), "valueKind": QueryJson(&self.node.kind), "operation": self.child(&self.node.operation)})
    }
}

impl Serialize for BodyJson<'_, hir::AtomicOperation<hir::DefaultExpressionV1>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::AtomicOperation::Load { order } => {
                object!(serializer, {"kind": "load", "order": QueryJson(&order.memory_order())})
            }
            hir::AtomicOperation::Store { value, order } => {
                object!(serializer, {"kind": "store", "value": self.child(value), "order": QueryJson(&order.memory_order())})
            }
            hir::AtomicOperation::Rmw {
                value,
                operation,
                order,
            } => {
                object!(serializer, {"kind": "rmw", "value": self.child(value), "operator": QueryJson(operation), "order": QueryJson(order)})
            }
            hir::AtomicOperation::CompareExchange {
                expected,
                value,
                order,
                result,
            } => {
                let (success, failure) = order.memory_orders();
                object!(serializer, {"kind": "compareExchange", "expected": self.child(expected), "value": self.child(value), "successOrder": QueryJson(&success), "failureOrder": QueryJson(&failure), "result": QueryJson(result)})
            }
        }
    }
}
