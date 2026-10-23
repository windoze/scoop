use super::*;

impl Serialize for BodyJson<'_, hir::DefaultLocalFunctionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut signature = self.child(self.node.function_type());
        if matches!(
            self.node.declaration(),
            identity::CallableTemplateOrigin::GenericFunction(_)
        ) {
            signature.introduced_binders =
                signature.introduced_binders.checked_add(1).ok_or_else(|| {
                    S::Error::custom("local generic signature binder depth overflows u32")
                })?;
        }
        object!(serializer, {
            "declaration": CallableDeclaration::from(self.node.declaration()),
            "definitionPath": QueryJson(self.node.definition_path()), "functionType": signature,
            "captures": self.child(self.node.captures()), "ownerTypeParameterCount": self.node.owner_type_parameter_count(),
        })
    }
}

macro_rules! lexical_callable {
    ($kind:ty) => {
        impl Serialize for BodyJson<'_, $kind> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                object!(serializer, {
                    "body": Reference::from(self.node.body()), "definitionPath": QueryJson(self.node.definition_path()),
                    "functionType": self.child(self.node.function_type()), "bodyTypeArguments": self.child(self.node.body_type_arguments()),
                    "captures": self.child(self.node.captures()), "ownerTypeParameterCount": self.node.owner_type_parameter_count(),
                })
            }
        }
    };
}
lexical_callable!(hir::DefaultLambdaV1);
lexical_callable!(hir::DefaultAnonymousFunctionV1);

impl Serialize for BodyJson<'_, hir::DefaultCallableBodyTypeArgumentsV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node.explicit_arguments() {
            None => {
                object!(serializer, {"kind": "lexical", "requirements": self.child(self.node.requirements())})
            }
            Some(arguments) => {
                object!(serializer, {"kind": "explicit", "arguments": self.child(arguments), "requirements": self.child(self.node.requirements())})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultCaptureV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "source": QueryJson(self.node.source()), "valueType": self.child(self.node.value_type()),
            "firstUseOrigin": QueryJson(self.node.first_use_origin()), "binding": QueryJson(self.node.binding()),
        })
    }
}

impl Serialize for QueryJson<'_, hir::DefaultCaptureSourceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::DefaultCaptureSourceV1::Local(local) => {
                object!(serializer, {"kind": "local", "local": QueryJson(local)})
            }
            hir::DefaultCaptureSourceV1::EnclosingCapture(index) => {
                object!(serializer, {"kind": "enclosingCapture", "index": index})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::DefaultCaptureBindingV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::DefaultCaptureBindingV1::Source => object!(serializer, {"kind": "source"}),
            hir::DefaultCaptureBindingV1::Definition {
                owner,
                scope,
                selector,
                definition,
            } => {
                object!(serializer, {"kind": "definition", "owner": QueryJson(owner), "scope": QueryJson(scope), "selector": QueryJson(selector), "definition": QueryJson(definition)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultCallableReferenceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "invoke": Reference::from(self.node.invoke()), "definitionPath": QueryJson(self.node.definition_path()),
            "target": self.child(self.node.target()), "functionType": self.child(self.node.function_type()),
            "captures": self.child(self.node.captures()), "ownerTypeParameterCount": self.node.owner_type_parameter_count(),
        })
    }
}

impl Serialize for BodyJson<'_, hir::DefaultCallableReferenceTargetV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultCallableReferenceTargetV1::Named(callee) => {
                object!(serializer, {"kind": "named", "callee": self.child(callee)})
            }
            hir::DefaultCallableReferenceTargetV1::Local {
                declaration,
                callee,
            } => {
                object!(serializer, {"kind": "local", "declaration": CallableDeclaration::from(*declaration), "callee": self.child(callee)})
            }
            hir::DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                object!(serializer, {"kind": "boundMember", "receiver": self.child(receiver), "callee": self.child(callee)})
            }
            hir::DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                object!(serializer, {"kind": "boundExtension", "receiver": self.child(receiver), "callee": self.child(callee)})
            }
        }
    }
}
