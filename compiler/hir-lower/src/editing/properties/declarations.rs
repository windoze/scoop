use super::*;
use crate::Owner;

impl Lowerer {
    pub(super) fn editing_property_declaration(
        &self,
        property: hir::PropertyId,
    ) -> V<hir::EditingPropertyDeclaration> {
        if self.property_type_pending(property) {
            return V::Unavailable(U::PendingInference);
        }
        let declaration = &self.properties[property];
        let owner = match declaration.owner {
            hir::PropertyOwner::Class(owner) => Some(Owner::Class(owner)),
            hir::PropertyOwner::Struct(owner) => Some(Owner::Struct(owner)),
            hir::PropertyOwner::Enum(owner) => Some(Owner::Enum(owner)),
            hir::PropertyOwner::Interface(owner) => Some(Owner::Interface(owner)),
            hir::PropertyOwner::Object(owner) => Some(Owner::Object(owner)),
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => None,
        };
        let extension = match declaration.owner {
            hir::PropertyOwner::Extension(extension) => Some(&self.extension_properties[extension]),
            _ => None,
        };
        let binders = hir::SemanticDeclarationBinders {
            owner: owner.map_or_else(Vec::new, |owner| self.owner_type_params(owner)),
            callable: extension.map_or_else(Vec::new, |extension| extension.type_params.clone()),
        };
        V::Known(hir::EditingPropertyDeclaration {
            property: Property::Current(property),
            name: declaration.name.clone(),
            mutable: declaration.capability.setter().is_some(),
            value_type: declaration.ty,
            receiver: extension.map(|extension| extension.receiver_ty),
            binders,
            origin: self.editing_property_origin(Property::Current(property)),
        })
    }
}
