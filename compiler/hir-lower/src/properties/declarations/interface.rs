use super::*;

impl Lowerer {
    pub(crate) fn allocate_interface_property(
        &mut self,
        owner: hir::InterfaceId,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
    ) {
        self.reject_logical_property_annotations("an interface property", &declaration.annotations);
        if declaration.receiver_ty.is_some() || !declaration.type_params.is_empty() {
            self.error(
                declaration.span,
                "extension properties may only be declared at top level".to_string(),
            );
            return;
        }
        if !matches!(
            declaration.body,
            ast::PropertyBodySyntax::Computed(_) | ast::PropertyBodySyntax::Abstract
        ) {
            self.error(
                declaration.span,
                format!(
                    "interface property `{}` cannot have storage, an initializer, or a delegate",
                    declaration.name.text
                ),
            );
            return;
        }
        let private = access.declared == hir::DeclaredVisibility::Private;
        let expected = self.next_property_id();
        let Some(capability) = self.allocate_property_accessors(
            expected,
            hir::PropertyOwner::Interface(owner),
            access.clone(),
            declaration,
            None,
            if private {
                hir::MethodModifier::Final
            } else {
                hir::MethodModifier::Open
            },
        ) else {
            return;
        };
        if access.declared == hir::DeclaredVisibility::Private {
            let getter = self.property_getters[capability.getter()].implementation;
            let setter = capability
                .setter()
                .map(|setter| self.property_setters[setter].implementation);
            if matches!(getter, hir::PropertyAccessorImplementation::AbstractSlot(_))
                || setter.is_some_and(|implementation| {
                    matches!(
                        implementation,
                        hir::PropertyAccessorImplementation::AbstractSlot(_)
                    )
                })
            {
                self.error(
                    declaration.name.span,
                    format!(
                        "private interface property `{}` must provide every accessor body",
                        declaration.name.text
                    ),
                );
            }
        }
        let all_abstract =
            std::iter::once(self.property_getters[capability.getter()].implementation)
                .chain(
                    capability
                        .setter()
                        .map(|setter| self.property_setters[setter].implementation),
                )
                .all(|implementation| {
                    matches!(
                        implementation,
                        hir::PropertyAccessorImplementation::AbstractSlot(_)
                    )
                });
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::Interface(owner),
            name: declaration.name.text.clone(),
            access,
            modifier: if private {
                hir::MethodModifier::Final
            } else if all_abstract {
                hir::MethodModifier::Abstract
            } else {
                hir::MethodModifier::Open
            },
            is_override: declaration.is_override,
            overrides: Vec::new(),
            override_access: Vec::new(),
            ty,
            capability,
            representation: hir::PropertyRepresentation::AccessorOnly,
            span: declaration.span,
        });
        assert_eq!(property, expected);
        self.interfaces[owner].properties.push(property);
    }
}
