use super::*;

impl Lowerer {
    pub(super) fn probe_generic_extension_property(
        mut self,
        binding: &hir::DirectImportedTargetBinding,
        property: &hir::ImportedDependencyPropertyCandidate,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Result<ImportedDependencyExtensionPropertyProbe, Box<Lowerer>> {
        let getter = self
            .dependencies
            .as_ref()
            .expect("an imported extension property has a dependency catalog")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Accessor(
                property.interface().accessors().getter(),
            ))
            .expect("a validated extension property retains its getter declaration");
        let probe = self.probe_imported_extension_accessor(getter, receiver, name)?;
        let selected = self.commit_imported_extension_accessor_signature(probe);
        Ok(ImportedDependencyExtensionPropertyProbe {
            target: ImportedExtensionPropertyTarget {
                binding: binding.clone(),
                accessors: ImportedExtensionPropertyAccessors::Generic {
                    getter: selected.template,
                    arguments: selected.arguments,
                },
            },
            receiver_type: selected.receiver.ty,
            receiver: selected.receiver,
            static_receiver_type: selected.static_receiver_type,
            value_type: selected.value_type,
            has_setter: property.interface().accessors().setter().is_some(),
            declaration_file: selected.declaration_file,
            declaration_span: selected.declaration_span,
            state: Box::new(self),
        })
    }

    pub(crate) fn lower_selected_imported_extension_property_read(
        &mut self,
        target: &ImportedExtensionPropertyTarget,
        receiver: PropertyCallReceiver,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        match &target.accessors {
            ImportedExtensionPropertyAccessors::ParameterFree => self
                .lower_imported_dependency_property_read(&target.binding, Some(receiver), span)
                .map(|read| read.expression),
            ImportedExtensionPropertyAccessors::Generic { getter, arguments } => {
                Some(self.emit_generic_extension_accessor(*getter, arguments, receiver, None, span))
            }
        }
    }

    pub(crate) fn lower_selected_imported_extension_property_write(
        &mut self,
        target: &ImportedExtensionPropertyTarget,
        receiver: PropertyCallReceiver,
        value: hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let ImportedExtensionPropertyAccessors::Generic { getter, arguments } = &target.accessors
        else {
            return self.lower_imported_dependency_property_write(
                &target.binding,
                Some(receiver),
                value,
                name,
                span,
            );
        };
        let hir::ImportedCallableTemplateOrigin::ExtensionAccessor(getter_id) =
            self.imported_generic_templates[*getter].declaration
        else {
            unreachable!("a selected generic extension getter retains its accessor identity")
        };
        let accessors = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_for_accessor(getter_id))
            .expect("a selected accessor belongs to a complete property declaration")
            .accessors();
        let Some(setter_id) = accessors.setter() else {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        };
        let setter = self
            .dependencies
            .as_ref()
            .expect("an imported extension property has a dependency catalog")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Accessor(setter_id))
            .expect("a writable property retains its setter declaration");
        if !self.imported_callable_is_accessible(setter.interface(), Some(receiver.static_type)) {
            self.error(
                name.span,
                format!("setter of property `{}` is not accessible", name.text),
            );
            return None;
        }
        let template = match self.request_imported_generic_template(setter) {
            Ok(template) => template,
            Err(error) => {
                self.error(name.span, error);
                return None;
            }
        };
        Some(hir::StatementKind::Expr(
            self.emit_generic_extension_accessor(template, arguments, receiver, Some(value), span),
        ))
    }

    fn emit_generic_extension_accessor(
        &mut self,
        template: hir::ImportedGenericCallableTemplateId,
        arguments: &hir::NonEmptyVec<hir::TypeId>,
        receiver: PropertyCallReceiver,
        value: Option<hir::Expr>,
        span: ast::Span,
    ) -> hir::Expr {
        let signature = self.imported_generic_templates[template].signature.clone();
        if signature.attributes.safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(span, "accessing an unsafe dependency property");
        }
        let bindings = signature
            .type_parameters
            .ids()
            .into_iter()
            .zip(arguments.iter().copied())
            .collect::<Vec<_>>();
        let result_type = self.instantiate_method_ty(signature.return_ty, &bindings);
        let source_receiver = hir::SourceCallReceiver::Receiver {
            static_type: receiver.static_type,
        };
        let mut args = vec![receiver.value];
        args.extend(value);
        let application =
            self.imported_generic_applications
                .alloc(hir::ImportedGenericCallableApplication {
                    template,
                    arguments: hir::ImportedCallableArguments::Function(arguments.to_vec()),
                });
        hir::Expr {
            kind: hir::ExprKind::ImportedGenericCall {
                application,
                kind: hir::ImportedGenericCallKind::Ordinary,
                binding: None,
                args,
                receiver: source_receiver,
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        }
    }
}
