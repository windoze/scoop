use super::*;

impl Serialize for BodyJson<'_, hir::SemanticConstructorBody> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let body = &self.node.declaration;
        object!(serializer, {
            "declaration": self.child(body.declaration()),
            "inputs": self.child(body.inputs().records()),
            "effects": QueryJson(&body.effects()),
            "predicates": self.child(body.predicates()),
            "origin": QueryJson(body.definition_origin()),
            "initialization": self.child(body.kind()),
            "commonInitialization": self.child(self.node.common.as_ref()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::ExportConstructorInitializationKindV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::ExportConstructorInitializationKindV1 as Kind;
        match self.node {
            Kind::StructPrimary => object!(serializer, {"kind": "structPrimary"}),
            Kind::StructSecondary { delegation, body } => {
                object!(serializer, {"kind": "structSecondary", "delegation": self.child(delegation), "body": self.child(body)})
            }
            Kind::ClassPrimary {
                base,
                primary_stores,
            } => {
                object!(serializer, {"kind": "classPrimary", "base": self.child(base), "primaryStores": self.child(primary_stores)})
            }
            Kind::ClassSecondaryThis { delegation, body } => {
                object!(serializer, {"kind": "classSecondaryThis", "delegation": self.child(delegation), "body": self.child(body)})
            }
            Kind::ClassSecondaryTerminal { base, body } => {
                object!(serializer, {"kind": "classSecondaryTerminal", "base": self.child(base), "body": self.child(body)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::ExportConstructorDelegationV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"target": self.child(&self.node.target), "arguments": self.child(&self.node.arguments)})
    }
}

impl Serialize for BodyJson<'_, hir::ExportPrimaryFieldStoreV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"field": self.child(&self.node.field), "parameter": QueryJson(&self.node.parameter)})
    }
}

impl Serialize for BodyJson<'_, hir::ExportCommonInitializationStepV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::ExportCommonInitializationStepV1::Field { field, value } => {
                object!(serializer, {"kind": "field", "field": self.child(field), "value": self.child(value)})
            }
            hir::ExportCommonInitializationStepV1::Body(body) => {
                object!(serializer, {"kind": "body", "body": self.child(body)})
            }
        }
    }
}
