use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_imported_qualified_call(
        &mut self,
        owner: ImportedNominalQualifier,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(ty) = owner.applied()
            && let Some(index) = self.find_imported_variant(ty, &name.text)
        {
            return self.lower_imported_variant_construct(ty, index, name, call, sink, Some(ty));
        }
        let mut bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Type,
            &name.text,
        );
        if bindings.is_empty()
            && let Some(nested) = self
                .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
                .ok()?
        {
            let declaration = self
                .imported_nominal_declaration(nested)
                .expect("a dependency nested type retains its declaration");
            return self.lower_imported_nominal_construct(declaration, name, call, sink, expected);
        }
        if bindings.is_empty()
            && let Some(ty) = self
                .imported_generic_companion_type(owner, name.span)
                .ok()?
        {
            let receiver = self.lower_imported_singleton_type(ty, name.span)?;
            return self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers::default(),
            );
        }
        if bindings.is_empty()
            && let Some(value) = owner
                .applied()
                .and_then(|ty| self.imported_nominal_declaration(ty))
                .and_then(|owner| self.imported_object_value(owner))
        {
            let receiver = self.lower_imported_singleton(value, name.span)?;
            return self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers::default(),
            );
        }
        bindings.extend(self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        ));
        if bindings.is_empty() {
            self.imported_static_member_error(owner, name);
            return None;
        }
        let targets = bindings
            .into_iter()
            .map(|binding| NamedCallBinding {
                target: NamedCallTarget::ImportedDependency(binding.target()),
                origin: NamedCallOrigin::Dependency(binding),
            })
            .collect::<Vec<_>>();
        self.lower_named_function_partition(
            &targets,
            ImportLookupLayer::Exact,
            &ast::CallExpr {
                callee: name.clone(),
                type_args: call.type_args.to_vec(),
                args: call.args.to_vec(),
                span: call.span,
            },
            sink,
            expected,
        )
        .ok()
        .flatten()
    }

    pub(super) fn imported_static_member_error(
        &mut self,
        owner: ImportedNominalQualifier,
        name: &ast::Ident,
    ) {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| {
                dependencies.nominal_declaration(self.imported_qualifier_owner(owner))
            })
            .expect("a static qualifier retains its dependency declaration");
        let owner_name = owner
            .applied()
            .map(|ty| self.type_name(ty))
            .unwrap_or_else(|| declaration.name().to_owned());
        let message = if matches!(
            declaration.interface.source_shape(),
            hir::NominalSourceShapeV1::Enum(_)
        ) {
            format!("enum `{}` has no variant `{}`", owner_name, name.text)
        } else {
            format!(
                "type `{}` has no accessible static member `{}`",
                owner_name, name.text
            )
        };
        self.error(name.span, message);
    }
}
