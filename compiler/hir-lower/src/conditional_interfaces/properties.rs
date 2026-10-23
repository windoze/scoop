use super::*;

impl Lowerer {
    pub(super) fn resolve_conditional_property(
        &mut self,
        owner: Owner,
        condition: hir::ConditionalInterfaceId,
        property: &ast::PropertyDecl,
    ) {
        let properties = match owner {
            Owner::Class(id) => &self.classes[id].properties,
            Owner::Struct(id) => &self.structs[id].properties,
            Owner::Enum(id) => &self.enums[id].properties,
            Owner::Object(id) => &self.classes[self.objects[id].backing_class].properties,
            Owner::Interface(_) => unreachable!("implementation owners are ordinary types"),
        };
        if properties
            .iter()
            .any(|&id| self.properties[id].name == property.name.text)
        {
            self.error(
                property.name.span,
                format!(
                    "duplicate property `{}` in {}",
                    property.name.text,
                    owner.describe(self)
                ),
            );
            return;
        }
        let Some(ty) = self.resolve_property_annotation(property) else {
            return;
        };
        let access = self.member_access(
            property.visibility,
            property.name.span,
            "property",
            owner,
            self.current_file,
            crate::visibility::MemberSlotAccess::Override,
        );
        let start = self.property_accessor_sources.len();
        match owner {
            Owner::Class(id) => {
                self.allocate_class_property(id, property, ty, access);
            }
            Owner::Object(id) => {
                self.allocate_object_property(
                    id,
                    property,
                    ty,
                    access,
                    crate::imports::PropertyImportSource::OutsideCurrentUnitSurface,
                );
            }
            Owner::Struct(_) | Owner::Enum(_) => {
                self.allocate_value_property(owner, property, ty, access)
            }
            Owner::Interface(_) => unreachable!("implementation owners are ordinary types"),
        }
        let availability = hir::ConditionalInterfaceGuard {
            declaration: condition,
            owner: self.owner_ty(owner),
        };
        for source in &self.property_accessor_sources[start..] {
            self.functions[source.function]
                .method
                .as_mut()
                .expect("conditional accessors are methods")
                .availability = Some(availability);
        }
    }
}
