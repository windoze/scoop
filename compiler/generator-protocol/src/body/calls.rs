use super::*;

impl Serialize for BodyJson<'_, hir::DefaultCallableRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "declaration": CallableDeclaration::from(self.node.declaration()), "owner": self.child(self.node.owner()),
            "typeArguments": self.child(self.node.type_arguments()), "requirements": self.child(self.node.requirements()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::DefaultMethodCalleeV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultMethodCalleeV1::Callable(callable) => {
                object!(serializer, {"kind": "callable", "callable": self.child(callable)})
            }
            hir::DefaultMethodCalleeV1::Bound(bound) => {
                object!(serializer, {"kind": "bound", "bound": self.child(bound)})
            }
            hir::DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                object!(serializer, {"kind": "derivedEquality", "ownerType": self.child(owner_type)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultBoundCallableRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"receiverType": self.child(self.node.receiver_type()), "source": self.child(self.node.source()), "instantiatedSignature": self.child(self.node.instantiated_signature())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultBoundCallableSourceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultBoundCallableSourceV1::Class { bound, callable } => {
                object!(serializer, {"kind": "class", "bound": self.child(bound), "callable": self.child(callable)})
            }
            hir::DefaultBoundCallableSourceV1::Interface { bound, member } => {
                object!(serializer, {"kind": "interface", "bound": self.child(bound), "member": CallableDeclaration::from(*member)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::SourceCallReceiver<identity::SignatureTypeKey>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::SourceCallReceiver::NoReceiver => serializer.serialize_none(),
            hir::SourceCallReceiver::Receiver { static_type } => {
                object!(serializer, {"staticType": self.child(static_type)})
            }
        }
    }
}
