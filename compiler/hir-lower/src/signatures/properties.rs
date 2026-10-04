use super::*;

impl Lowerer {
    pub(super) fn resolve_struct_properties(
        &mut self,
        id: StructId,
        decl: &ast::StructDecl,
        seen: &mut HashSet<String>,
    ) {
        for property in decl.members.iter().filter_map(|member| match member {
            ast::StructMember::Property(property) => Some(property.as_ref()),
            _ => None,
        }) {
            if !seen.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate property `{}` in struct `{}`",
                        property.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&property.ty) else {
                continue;
            };
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Struct(id),
                self.current_file,
                if property.is_override {
                    crate::visibility::MemberSlotAccess::Override
                } else {
                    crate::visibility::MemberSlotAccess::None
                },
            );
            self.allocate_value_property(Owner::Struct(id), property, ty, access);
        }
    }
}
