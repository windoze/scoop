use scoop_ast as ast;
use scoop_hir as hir;

use crate::visibility::MemberSlotAccess;
use crate::{Lowerer, Owner};

impl Lowerer {
    pub(super) fn property_setter_access(
        &mut self,
        declaration: &ast::PropertyDecl,
        setter: Option<&ast::SetterDecl>,
        owner: hir::PropertyOwner,
        property_access: hir::DeclarationAccess,
    ) -> hir::DeclarationAccess {
        let Some(setter) = setter else {
            return property_access;
        };
        let ast::SetterVisibilitySyntax::Explicit { visibility, span } = setter.visibility else {
            return property_access;
        };
        let syntax = ast::VisibilitySyntax::Explicit { visibility, span };
        let member_owner = match owner {
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => None,
            hir::PropertyOwner::Class(owner) => Some(Owner::Class(owner)),
            hir::PropertyOwner::Struct(owner) => Some(Owner::Struct(owner)),
            hir::PropertyOwner::Enum(owner) => Some(Owner::Enum(owner)),
            hir::PropertyOwner::Interface(owner) => Some(Owner::Interface(owner)),
            hir::PropertyOwner::Object(owner) => Some(Owner::Object(owner)),
        };
        let access = if let Some(owner) = member_owner {
            let slot = if property_access.slot.is_none() {
                MemberSlotAccess::None
            } else if declaration.is_override {
                MemberSlotAccess::Override
            } else {
                MemberSlotAccess::Declared
            };
            self.member_access(
                syntax,
                span,
                "property setter",
                owner,
                self.current_file,
                slot,
            )
        } else {
            self.top_level_access(syntax, span, "property setter", self.current_file)
        };
        if !self.access_domain_is_subset(&access.lookup.0, &property_access.lookup.0) {
            self.error(
                span,
                format!(
                    "setter of property `{}` cannot be more visible than the property",
                    declaration.name.text
                ),
            );
        }
        access
    }
}
