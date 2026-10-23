use super::*;
use crate::Reference;
use scoop_identity::BindableEntity;

impl Serialize for QueryJson<'_, BindableEntity> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self.0 {
            BindableEntity::Type(id) => {
                object!(serializer, {"kind": "type", "id": Reference::from(id)})
            }
            BindableEntity::GenericType(id) => {
                object!(serializer, {"kind": "genericType", "id": Reference::from(id)})
            }
            BindableEntity::ObjectValue(id) => {
                object!(serializer, {"kind": "objectValue", "id": Reference::from(id)})
            }
            BindableEntity::Function(id) => {
                object!(serializer, {"kind": "function", "id": Reference::from(id)})
            }
            BindableEntity::GenericFunction(id) => {
                object!(serializer, {"kind": "genericFunction", "id": Reference::from(id)})
            }
            BindableEntity::Property(id) => {
                object!(serializer, {"kind": "property", "id": Reference::from(id)})
            }
            BindableEntity::ExtensionProperty(id) => {
                object!(serializer, {"kind": "extensionProperty", "id": Reference::from(id)})
            }
            BindableEntity::TypeAlias(id) => {
                object!(serializer, {"kind": "typeAlias", "id": Reference::from(id)})
            }
            BindableEntity::EnumVariant(id) => {
                object!(serializer, {"kind": "enumVariant", "id": Reference::from(id)})
            }
            BindableEntity::Annotation(id) => {
                object!(serializer, {"kind": "annotation", "id": Reference::from(id)})
            }
        }
    }
}
