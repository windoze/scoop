use super::*;

impl Serialize for BodyJson<'_, hir::ExportGenericCallableBodyV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let body = self.node;
        object!(serializer, {
            "owner": CallableDeclaration::from(body.owner()),
            "locals": self.child(body.locals().records()),
            "parameters": QueryJson(body.parameters()),
            "statements": self.child(body.statements()), "result": self.child(body.result()),
            "effects": QueryJson(&body.effects()),
            "typeParameters": self.child(body.type_parameters().arguments()),
            "predicates": self.child(body.predicates()),
            "origin": QueryJson(body.definition_origin()),
            "captureTypes": self.child(body.capture_types()),
            "contextParameters": self.child(body.context_parameters()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::ExportDefaultTemplateV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let body = self.node;
        let arguments = self.application.map_or_else(
            || body.type_parameters().arguments(),
            |application| application.arguments().arguments(),
        );
        object!(serializer, {
            "key": QueryJson(&body.key()),
            "definitionRoot": CallableDeclaration::from(body.definition_root().declaration()),
            "definitionPath": QueryJson(body.definition_path()),
            "locals": self.child(body.locals().records()),
            "body": self.child(body.body()), "result": self.child(body.result()),
            "allowsSuspend": body.allows_suspend().value(),
            "typeArguments": QueryJson(arguments),
            "receiver": self.child(body.receiver()),
            "previousParameters": QueryJson(body.value_parameters().parameters()),
            "origin": QueryJson(body.definition_origin()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::ExportDefaultBodyV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"statements": self.child(self.node.statements()), "value": self.child(self.node.value())})
    }
}

impl Serialize for BodyJson<'_, hir::ExportTemplateFragmentV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "locals": self.child(self.node.locals().records()),
            "statements": self.child(self.node.statements()),
            "results": self.child(self.node.results()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::OptionalTemplateReceiverV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node.receiver() {
            Some(receiver) => {
                object!(serializer, {"local": QueryJson(receiver.local()), "valueType": self.child(receiver.value_type())})
            }
            None => serializer.serialize_none(),
        }
    }
}

impl Serialize for QueryJson<'_, hir::TemplateValueParameterV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"position": self.0.position(), "local": QueryJson(self.0.local())})
    }
}

impl Serialize for QueryJson<'_, hir::ExportDefaultTemplateKeyV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"owner": CallableDeclaration::from(self.0.owner()), "parameter": self.0.parameter_position()})
    }
}

impl Serialize for BodyJson<'_, hir::GenericTemplatePredicatesV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"noGc": self.child(self.node.no_gc().arguments()), "gcFreePointees": self.child(self.node.gc_free_pointees().arguments())})
    }
}

impl Serialize for BodyJson<'_, hir::SourceParameterShapeV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"name": self.node.name().as_str(), "valueType": self.child(self.node.value_type())})
    }
}
