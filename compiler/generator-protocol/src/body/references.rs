use super::*;

impl Serialize for BodyJson<'_, hir::DefaultEnumVariantRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"declaration": Reference::from(self.node.declaration()), "ownerType": self.child(self.node.owner_type())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultEnumVariantFieldRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"declaration": Reference::from(self.node.declaration()), "ownerType": self.child(self.node.owner_type())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultFieldRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultFieldRefV1::*;
        match self.node {
            Struct {
                declaration,
                owner_type,
            } => {
                object!(serializer, {"kind": "struct", "declaration": Reference::from(*declaration), "ownerType": self.child(owner_type)})
            }
            Class {
                declaration,
                owner_type,
            } => {
                object!(serializer, {"kind": "class", "declaration": Reference::from(*declaration), "ownerType": self.child(owner_type)})
            }
            Tuple { declaration_index } => {
                object!(serializer, {"kind": "tuple", "index": declaration_index})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultConstructorRefV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultConstructorRefV1::*;
        match self.node {
            Struct {
                declaration,
                owner_type,
            } => {
                object!(serializer, {"kind": "struct", "declaration": Reference::from(*declaration), "ownerType": self.child(owner_type)})
            }
            Class {
                declaration,
                owner_type,
            } => {
                object!(serializer, {"kind": "class", "declaration": QueryJson(declaration), "ownerType": self.child(owner_type)})
            }
            Variant {
                declaration,
                owner_type,
            } => {
                object!(serializer, {"kind": "variant", "declaration": Reference::from(*declaration), "ownerType": self.child(owner_type)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::DefaultClassConstructorIdV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::DefaultClassConstructorIdV1::Source(id) => {
                object!(serializer, {"kind": "constructor", "id": Reference::from(*id)})
            }
            hir::DefaultClassConstructorIdV1::Generated(id) => {
                object!(serializer, {"kind": "generated", "id": Reference::from(*id)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultGenericDelegateReferenceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"property": Reference::from(self.node.property()), "arguments": self.child(self.node.arguments())})
    }
}
