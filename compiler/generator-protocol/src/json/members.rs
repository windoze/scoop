use super::QueryJson;
use crate::{FieldId, NominalId, Reference};
use scoop_hir as hir;
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, hir::DeclaredVisibilityV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            hir::DeclaredVisibilityV1::Public => "public",
            hir::DeclaredVisibilityV1::Internal => "internal",
            hir::DeclaredVisibilityV1::Private => "private",
            hir::DeclaredVisibilityV1::Protected => "protected",
        })
    }
}

impl Serialize for QueryJson<'_, hir::AccessDomain> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"inhabited": !self.0.is_empty(), "constraints": QueryJson(self.0.constraints())})
    }
}

impl Serialize for QueryJson<'_, hir::AccessConstraint> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::AccessConstraint::Cone(cone) => {
                object!(serializer, {"kind": "cone", "cone": Reference::from(*cone)})
            }
            hir::AccessConstraint::File(source) => {
                object!(serializer, {"kind": "file", "source": QueryJson(source)})
            }
            hir::AccessConstraint::LexicalOwner(owner) => {
                object!(serializer, {"kind": "lexicalOwner", "owner": NominalId::from(*owner)})
            }
            hir::AccessConstraint::SubclassesOf(owner) => {
                object!(serializer, {"kind": "subclassesOf", "owner": NominalId::from(*owner)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::SemanticMemberAccess> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"visibility": QueryJson(&self.0.visibility), "domain": QueryJson(&self.0.domain), "accessible": self.0.accessible})
    }
}

impl Serialize for QueryJson<'_, hir::NominalFieldStorage> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::NominalFieldStorage::Declared => object!(serializer, {"kind": "declared"}),
            hir::NominalFieldStorage::EnclosingInstance => {
                object!(serializer, {"kind": "enclosingInstance"})
            }
            hir::NominalFieldStorage::CapturedEnvironment => {
                object!(serializer, {"kind": "capturedEnvironment"})
            }
            hir::NominalFieldStorage::PropertyBacking(property) => {
                object!(serializer, {"kind": "propertyBacking", "property": Reference::from(*property)})
            }
            hir::NominalFieldStorage::PropertyDelegate(property) => {
                object!(serializer, {"kind": "propertyDelegate", "property": Reference::from(*property)})
            }
            hir::NominalFieldStorage::Generated => object!(serializer, {"kind": "generated"}),
        }
    }
}

impl Serialize for QueryJson<'_, hir::StaticParameterTarget> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::StaticParameterTarget::Plain => object!(serializer, {"kind": "plain"}),
            hir::StaticParameterTarget::Field(field) => {
                object!(serializer, {"kind": "field", "field": FieldId::from(*field)})
            }
            hir::StaticParameterTarget::Property(property) => {
                object!(serializer, {"kind": "property", "property": Reference::from(*property)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::SemanticConstructorKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::SemanticConstructorKind::Primary => object!(serializer, {"kind": "primary"}),
            hir::SemanticConstructorKind::Secondary => object!(serializer, {"kind": "secondary"}),
            hir::SemanticConstructorKind::Variant(variant) => {
                object!(serializer, {"kind": "variant", "variant": Reference::from(*variant)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::VariantStyle> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            hir::VariantStyle::Unit => "unit",
            hir::VariantStyle::Positional => "positional",
            hir::VariantStyle::Named => "named",
            hir::VariantStyle::Constructor => "constructor",
        })
    }
}

impl Serialize for QueryJson<'_, hir::CallableModalityV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            hir::CallableModalityV1::Final => "final",
            hir::CallableModalityV1::Open => "open",
            hir::CallableModalityV1::Abstract => "abstract",
            hir::CallableModalityV1::InterfaceDefault => "interfaceDefault",
        })
    }
}

impl Serialize for QueryJson<'_, hir::PropertyAccessorImplementationV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            hir::PropertyAccessorImplementationV1::Storage => "storage",
            hir::PropertyAccessorImplementationV1::StorageBody => "storageBody",
            hir::PropertyAccessorImplementationV1::Constant => "constant",
            hir::PropertyAccessorImplementationV1::Body => "body",
            hir::PropertyAccessorImplementationV1::AbstractSlot => "abstractSlot",
        })
    }
}

impl Serialize for QueryJson<'_, hir::StaticNominalKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = match self.0 {
            hir::StaticNominalKind::Struct => "struct",
            hir::StaticNominalKind::Enum => "enum",
            hir::StaticNominalKind::Class => "class",
            hir::StaticNominalKind::Object => "object",
            hir::StaticNominalKind::Interface => "interface",
            hir::StaticNominalKind::Intrinsic(kind) => {
                return object!(serializer, {"kind": "intrinsic", "intrinsic": kind.name()});
            }
        };
        object!(serializer, {"kind": kind})
    }
}
