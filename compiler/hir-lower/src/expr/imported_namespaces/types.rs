use super::*;

impl Lowerer {
    pub(super) fn resolve_imported_nested_type(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
    ) -> Result<Option<TypeId>, ()> {
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Type, &name.text);
        match bindings.as_slice() {
            [] => {
                let declaration = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nested_nominal(owner, &name.text))
                    .cloned();
                let Some(declaration) = declaration else {
                    return Ok(None);
                };
                if !self.access_domain_allows(&self.imported_nominal_access_domain(&declaration)) {
                    self.error(
                        name.span,
                        format!(
                            "type `{}` is not accessible from this source location",
                            name.text
                        ),
                    );
                    return Err(());
                }
                self.resolve_imported_nominal_owner_arguments(declaration.owner(), name, arguments)
                    .map(Some)
                    .ok_or(())
            }
            [binding] => {
                let ty = if arguments.is_empty() {
                    self.resolve_imported_dependency_type_target(binding, name, false)
                } else {
                    self.resolve_imported_generic_type_target(binding, name, arguments)
                };
                ty.map(Some).ok_or(())
            }
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }

    pub(in crate::expr) fn resolve_imported_nominal_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<ImportedNominalQualifier>, ()> {
        if let Some(reference) = expression.applied_qualifier_type() {
            self.check_applied_qualifier_shadowing(&reference)?;
            let ty = self.resolve_type_ref(&reference).ok_or(())?;
            return Ok(self
                .imported_nominal_owner(ty)
                .map(|_| ImportedNominalQualifier::Applied(ty)));
        }
        let owner = match expression {
            ast::Expr::Var(name) => {
                if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
                    || self.lexical_nested_nominal_target(&name.text).is_some()
                {
                    return Ok(None);
                }
                match self.lookup_expression_qualifier(name) {
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(
                            hir::ImportedTarget::GenericType(owner),
                        ),
                    ) => return Ok(Some(ImportedNominalQualifier::Generic(owner.persistent()))),
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(_),
                    ) => self
                        .resolve_type_ref(&ast::TypeRef {
                            kind: ast::TypeRefKind::Named(name.clone()),
                            span: name.span,
                        })
                        .ok_or(())?,
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyObject(value),
                    ) => self.imported_singleton_type(value).map_err(|error| {
                        self.error(
                            name.span,
                            format!("invalid dependency singleton: {error:?}"),
                        )
                    })?,
                    ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    ))
                    | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => self
                        .resolve_type_alias_id_reference(alias, name, false)
                        .ok_or(())?,
                    _ => return Ok(None),
                }
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    return Ok(None);
                };
                let Some(owner) = self.resolve_imported_nominal_qualifier(&access.receiver)? else {
                    return Ok(None);
                };
                let bindings = self.imported_static_bindings(
                    self.imported_qualifier_owner(owner),
                    BindingNamespace::Type,
                    &name.text,
                );
                if let Some(host) = owner.applied()
                    && let [binding] = bindings.as_slice()
                    && let Some(nested) = binding.target().source_nominal()
                    && self.nominal_is_companion(nested)
                {
                    return self
                        .apply_companion_type(host, nested, access.span)
                        .map(|ty| Some(ImportedNominalQualifier::Applied(ty)))
                        .ok_or(());
                }
                if let [binding] = bindings.as_slice()
                    && let hir::ImportedTarget::GenericType(owner) = binding.target()
                {
                    return Ok(Some(ImportedNominalQualifier::Generic(owner.persistent())));
                }
                return self
                    .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
                    .map(|ty| ty.map(ImportedNominalQualifier::Applied));
            }
            _ => return Ok(None),
        };
        Ok(self
            .imported_nominal_owner(owner)
            .map(|_| ImportedNominalQualifier::Applied(owner)))
    }
}
