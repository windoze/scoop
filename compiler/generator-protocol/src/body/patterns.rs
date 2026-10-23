use super::*;

impl Serialize for BodyJson<'_, hir::DefaultPatternV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultPatternViewV1::*;
        match self.node.view() {
            Binding { local } => {
                object!(serializer, {"kind": "binding", "local": QueryJson(local)})
            }
            Wildcard => object!(serializer, {"kind": "wildcard"}),
            Literal {
                value,
                equality,
                subject_type,
            } => {
                object!(serializer, {"kind": "literal", "value": self.child(value), "equality": self.child(equality), "subjectType": self.child(subject_type)})
            }
            Variant { variant, fields } => {
                object!(serializer, {"kind": "variant", "variant": self.child(variant), "fields": self.child(fields)})
            }
            Tuple { elements } => {
                object!(serializer, {"kind": "tuple", "elements": self.child(elements)})
            }
            Struct { owner_type, fields } => {
                object!(serializer, {"kind": "struct", "ownerType": self.child(owner_type), "fields": self.child(fields)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultPatternFieldV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"index": self.node.declaration_index(), "pattern": self.child(self.node.pattern())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultLiteralEqualityV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultLiteralEqualityV1::Char => object!(serializer, {"kind": "char"}),
            hir::DefaultLiteralEqualityV1::Integer { kind } => {
                object!(serializer, {"kind": "integer", "integerKind": QueryJson(kind)})
            }
            hir::DefaultLiteralEqualityV1::Float { kind } => {
                object!(serializer, {"kind": "float", "floatKind": QueryJson(kind)})
            }
            hir::DefaultLiteralEqualityV1::Ordinary { target } => {
                object!(serializer, {"kind": "ordinary", "target": self.child(target)})
            }
        }
    }
}
