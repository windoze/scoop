use super::*;

impl Lowerer {
    pub(in crate::expr) fn imported_unit_variant_requires_expected(
        &self,
        access: &ast::FieldAccess,
    ) -> bool {
        if self.dependencies.is_none() || access.navigation != ast::Navigation::Direct {
            return false;
        }
        let ast::FieldSelector::Name(name) = &access.selector else {
            return false;
        };
        let mut root = access.receiver.as_ref();
        while let ast::Expr::FieldAccess(parent) = root {
            root = parent.receiver.as_ref();
        }
        let ast::Expr::Var(root) = root else {
            return false;
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&root.text)
            || self.lexical_nested_nominal_target(&root.text).is_some()
        {
            return false;
        }
        let mut probe = self.clone();
        let Ok(Some(ImportedNominalQualifier::Generic(owner))) =
            probe.resolve_imported_nominal_qualifier(&access.receiver)
        else {
            return false;
        };
        let bindings = probe.imported_static_bindings(
            hir::SourceNominalId::GenericTemplate(owner),
            BindingNamespace::Value,
            &name.text,
        );
        matches!(bindings.as_slice(), [binding]
            if probe.imported_variant_binding_requires_expected(binding))
    }

    pub(in crate::expr) fn lower_imported_qualified_field(
        &mut self,
        owner: ImportedNominalQualifier,
        access: &ast::FieldAccess,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let ast::FieldSelector::Name(name) = &access.selector else {
            self.error(
                access.span,
                "a type qualifier requires a named member".into(),
            );
            return None;
        };
        if let Some(ty) = owner.applied()
            && let Some(index) = self.find_imported_variant(ty, &name.text)
        {
            return self.lower_imported_unit_variant(ty, index, name);
        }
        let bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        );
        match bindings.as_slice() {
            [binding] => match binding.target() {
                hir::ImportedTarget::ObjectValue(value) => {
                    return self.lower_qualified_imported_object(
                        value.persistent(),
                        owner,
                        access.span,
                    );
                }
                hir::ImportedTarget::Property(_) => {
                    if let Some(ty) = self
                        .imported_generic_companion_type(owner, access.span)
                        .ok()?
                    {
                        let is_const = self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| dependencies.property_candidate(binding).ok())
                            .is_some_and(|property| {
                                property.interface().representation()
                                    == hir::PropertyRepresentationV1::Const
                            });
                        if !is_const {
                            let receiver =
                                self.lower_imported_singleton_type(ty, access.receiver.span())?;
                            return self
                                .lower_imported_member_property_read(
                                    &receiver,
                                    name,
                                    access.span,
                                    expected,
                                )
                                .ok()?;
                        }
                    }
                    return self
                        .lower_imported_dependency_property_read(binding, None, access.span)
                        .map(|read| read.expression);
                }
                hir::ImportedTarget::EnumVariant(_) => {
                    return self.lower_imported_variant_binding(binding, name, expected);
                }
                _ => {}
            },
            [] => {}
            _ => {
                self.error(
                    name.span,
                    format!("static member `{}` is not an unambiguous value", name.text),
                );
                return None;
            }
        }
        if let Some(ty) = self
            .imported_generic_companion_type(owner, access.span)
            .ok()?
        {
            let receiver = self.lower_imported_singleton_type(ty, access.receiver.span())?;
            if let Some(value) = self
                .lower_imported_member_property_read(&receiver, name, access.span, expected)
                .ok()?
            {
                return Some(value);
            }
        }
        if let Some(nested) = self
            .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
            .ok()?
            && let Some(value) = self
                .imported_nominal_declaration(nested)
                .and_then(|owner| self.imported_object_value(owner))
        {
            return self.lower_imported_singleton(value, access.span);
        }
        if let Some(value) = owner
            .applied()
            .and_then(|ty| self.imported_nominal_declaration(ty))
            .and_then(|owner| self.imported_object_value(owner))
        {
            let receiver = self.lower_imported_singleton(value, access.receiver.span())?;
            if let Some(value) = self
                .lower_imported_member_property_read(&receiver, name, access.span, expected)
                .ok()?
            {
                return Some(value);
            }
        }
        self.imported_static_member_error(owner, name);
        None
    }

    pub(crate) fn resolve_imported_qualified_property(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
    ) -> Result<Option<hir::DirectImportedTargetBinding>, ()> {
        let Some(owner) = self.resolve_imported_nominal_qualifier(receiver)? else {
            return Ok(None);
        };
        if self
            .imported_generic_companion_type(owner, receiver.span())?
            .is_some()
        {
            return Ok(None);
        }
        let bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        );
        match bindings.as_slice() {
            [binding] if matches!(binding.target(), hir::ImportedTarget::Property(_)) => {
                Ok(Some(binding.clone()))
            }
            _ => Ok(None),
        }
    }
}
