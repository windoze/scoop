use scoop_ast as ast;
use scoop_hir as hir;

use super::{
    BackingFieldContext, PropertyAccessorBodyKind, PropertyAccessorKind, PropertyAccessorSource,
};
use crate::annotations::FunctionTarget;
use crate::{Function, FunctionKind, Lowerer, Owner, TypeId};

mod accessors;
mod classes;
mod functions;
mod interface;
mod storage;

struct AccessorFunctionAllocation {
    property: hir::PropertyId,
    kind: PropertyAccessorKind,
    owner: hir::PropertyOwner,
    access: hir::DeclarationAccess,
    backing: Option<hir::PropertyBacking>,
    modifier: hir::MethodModifier,
}

impl Lowerer {
    pub(crate) fn resolve_interface_properties(
        &mut self,
        owner: hir::InterfaceId,
        declaration: &ast::InterfaceDecl,
    ) {
        self.type_params_in_scope = self.interfaces[owner].type_params.clone();
        let mut names = std::collections::HashSet::new();
        for property in &declaration.properties {
            if !names.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate property `{}` in interface `{}`",
                        property.name.text, declaration.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&property.ty) else {
                continue;
            };
            let private = matches!(
                property.visibility,
                ast::VisibilitySyntax::Explicit {
                    visibility: ast::DeclaredVisibility::Private,
                    ..
                }
            );
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Interface(owner),
                self.current_file,
                if private {
                    crate::visibility::MemberSlotAccess::None
                } else if property.is_override {
                    crate::visibility::MemberSlotAccess::Override
                } else {
                    crate::visibility::MemberSlotAccess::Declared
                },
            );
            if self.interfaces[owner].access.declared == hir::DeclaredVisibility::Public
                && !matches!(
                    access.declared,
                    hir::DeclaredVisibility::Public | hir::DeclaredVisibility::Private
                )
            {
                self.error(
                    property.name.span,
                    format!(
                        "member `{}` of public interface `{}` must be explicitly public",
                        property.name.text, declaration.name.text
                    ),
                );
            }
            if private && property.is_override {
                self.error(
                    property.name.span,
                    format!(
                        "private interface property `{}` cannot be an override",
                        property.name.text
                    ),
                );
            }
            self.allocate_interface_property(owner, property, ty, access);
        }
        self.type_params_in_scope.clear();
    }

    pub(crate) fn next_property_id(&self) -> hir::PropertyId {
        hir::PropertyId::from_raw((self.properties.len() as u32).into())
    }

    pub(crate) fn allocate_value_property(
        &mut self,
        owner: Owner,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
    ) {
        self.reject_logical_property_annotations("a value-type property", &declaration.annotations);
        if declaration.receiver_ty.is_some() || !declaration.type_params.is_empty() {
            self.error(
                declaration.span,
                "extension properties may only be declared at top level".to_string(),
            );
            return;
        }
        if declaration.mutable {
            self.error(
                declaration.name.span,
                format!(
                    "value-type property `{}` cannot be mutable",
                    declaration.name.text
                ),
            );
            return;
        }
        if declaration.modifier != ast::MethodModifier::Final {
            self.error(
                declaration.name.span,
                format!(
                    "value-type property `{}` cannot be open or abstract",
                    declaration.name.text
                ),
            );
            return;
        }
        if !matches!(declaration.body, ast::PropertyBodySyntax::Computed(_)) {
            self.error(
                declaration.span,
                format!(
                    "value-type property `{}` must be a computed val",
                    declaration.name.text
                ),
            );
            return;
        }
        let property_owner = match owner {
            Owner::Struct(owner) => hir::PropertyOwner::Struct(owner),
            Owner::Enum(owner) => hir::PropertyOwner::Enum(owner),
            Owner::Class(_) | Owner::Interface(_) => {
                unreachable!("value property owners are structs or enums")
            }
            Owner::Object(_) => unreachable!("an object is a reference-type property owner"),
        };
        let expected = self.next_property_id();
        let Some(capability) = self.allocate_property_accessors(
            expected,
            property_owner,
            access.clone(),
            declaration,
            None,
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let property = self.properties.alloc(hir::Property {
            owner: property_owner,
            name: declaration.name.text.clone(),
            access,
            modifier: hir::MethodModifier::Final,
            is_override: declaration.is_override,
            overrides: Vec::new(),
            ty,
            capability,
            representation: hir::PropertyRepresentation::AccessorOnly,
            span: declaration.span,
        });
        assert_eq!(property, expected);
        match owner {
            Owner::Struct(owner) => self.structs[owner].properties.push(property),
            Owner::Enum(owner) => self.enums[owner].properties.push(property),
            Owner::Class(_) | Owner::Interface(_) => {
                unreachable!("value property owners are structs or enums")
            }
            Owner::Object(_) => unreachable!("an object is a reference-type property owner"),
        }
    }
}
