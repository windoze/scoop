use super::*;

impl Serialize for BodyJson<'_, hir::DefaultStatementV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"operation": self.child(self.node.kind()), "origin": QueryJson(self.node.definition_origin())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultStatementKindV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultStatementKindV1::*;
        match self.node {
            MemberRequirementCall {
                contract,
                binding,
                arguments,
                result,
            } => {
                object!(serializer, {"kind": "memberRequirementCall", "contract": self.child(contract), "binding": self.child(binding), "arguments": self.child(arguments), "result": self.child(result)})
            }
            ContextScope { value, body } => {
                object!(serializer, {"kind": "contextScope", "value": self.child(value), "body": self.child(body)})
            }
            Expr(value) => object!(serializer, {"kind": "expr", "value": self.child(value)}),
            InitializationEnsure(unit) => {
                object!(serializer, {"kind": "initializationEnsure", "unit": Reference::from(*unit)})
            }
            GenericDelegateEnsure(delegate) => {
                object!(serializer, {"kind": "genericDelegateEnsure", "delegate": self.child(delegate)})
            }
            LocalFunction(function) => {
                object!(serializer, {"kind": "localFunction", "function": self.child(function)})
            }
            Return(value) => object!(serializer, {"kind": "return", "value": self.child(value)}),
            ValDecl { pattern, init } => {
                object!(serializer, {"kind": "valDecl", "pattern": self.child(pattern), "init": self.child(init)})
            }
            Assign { target, value } => {
                object!(serializer, {"kind": "assign", "target": self.child(target), "value": self.child(value)})
            }
            If {
                condition,
                then_body,
                else_body,
            } => {
                object!(serializer, {"kind": "if", "condition": self.child(condition), "thenBody": self.child(then_body), "elseBody": self.child(else_body)})
            }
            While {
                loop_kind,
                condition_setup,
                condition,
                body,
            } => {
                object!(serializer, {"kind": "while", "loopKind": match loop_kind { hir::LoopKind::While => "while", hir::LoopKind::DoWhile => "doWhile" }, "conditionSetup": self.child(condition_setup), "condition": self.child(condition), "body": self.child(body)})
            }
            Break(target) => object!(serializer, {"kind": "break", "target": target.depth()}),
            Continue(target) => object!(serializer, {"kind": "continue", "target": target.depth()}),
            When(value) => object!(serializer, {"kind": "when", "when": self.child(value)}),
            Try(value) => object!(serializer, {"kind": "try", "try": self.child(value)}),
            Throw(value) => object!(serializer, {"kind": "throw", "value": self.child(value)}),
        }
    }
}

impl Serialize for BodyJson<'_, hir::OptionalDefaultStatementListV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node.view() {
            hir::OptionalDefaultStatementListViewV1::Absent => serializer.serialize_none(),
            hir::OptionalDefaultStatementListViewV1::Present(statements) => {
                self.child(statements).serialize(serializer)
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultAssignTargetV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultAssignTargetV1::*;
        match self.node {
            GenericDelegateStorage(delegate) => {
                object!(serializer, {"kind": "genericDelegateStorage", "delegate": self.child(delegate)})
            }
            Local { local } => object!(serializer, {"kind": "local", "local": QueryJson(local)}),
            Global { property } => {
                object!(serializer, {"kind": "global", "property": Reference::from(*property)})
            }
            Index { array, index } => {
                object!(serializer, {"kind": "index", "array": self.child(array), "index": self.child(index)})
            }
            Field { receiver, field } => {
                object!(serializer, {"kind": "field", "receiver": self.child(receiver), "field": self.child(field)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::DefaultPlaceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::DefaultPlaceV1::Local { local } => {
                object!(serializer, {"kind": "local", "local": QueryJson(local)})
            }
            hir::DefaultPlaceV1::Global { property } => {
                object!(serializer, {"kind": "global", "property": Reference::from(*property)})
            }
        }
    }
}
