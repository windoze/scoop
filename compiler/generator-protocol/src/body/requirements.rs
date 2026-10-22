use super::*;

impl Serialize for QueryJson<'_, hir::DefaultMemberRequirementRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"declaration": CallableDeclaration::from(self.0.declaration), "parameter": self.0.parameter, "requirement": self.0.requirement})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultRequirementBindingV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"requirement": QueryJson(&self.node.requirement), "value": self.child(&self.node.value)})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultRequirementBindingValueV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultRequirementBindingValueV1::Forwarded(requirement) => {
                object!(serializer, {"kind": "forwarded", "requirement": QueryJson(requirement)})
            }
            hir::DefaultRequirementBindingValueV1::Implementation {
                body,
                inputs,
                type_arguments,
            } => object!(serializer, {
                "kind": "implementation", "body": BodyJson::new(body), "inputs": inputs,
                "typeArguments": self.child(type_arguments),
            }),
        }
    }
}

impl Serialize for BodyJson<'_, identity::MemberRequirementKey> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "subject": self.child(&self.node.subject), "call": self.child(&self.node.call),
            "arguments": self.child(&self.node.arguments), "result": self.child(&self.node.result), "effects": QueryJson(&self.node.effects),
        })
    }
}

impl Serialize for BodyJson<'_, identity::RequiredMemberKey<identity::SignatureTypeKey>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            identity::RequiredMemberKey::Constructor => {
                object!(serializer, {"kind": "constructor"})
            }
            identity::RequiredMemberKey::Method {
                name,
                type_arguments,
            } => {
                object!(serializer, {"kind": "method", "name": name, "typeArguments": self.child(type_arguments)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, identity::RequiredArgumentKey<identity::SignatureTypeKey>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"name": self.node.name, "spread": self.node.spread, "valueType": self.child(&self.node.ty)})
    }
}
