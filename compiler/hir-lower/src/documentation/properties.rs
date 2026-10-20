use super::*;

impl Lowerer {
    pub(crate) fn document_property(&mut self, id: hir::PropertyId, source: &ast::PropertyDecl) {
        if self.documentation.is_none() {
            return;
        }
        let target = Source::Property(id);
        let mut signature = signatures::prefix(source.span, source.name.span);
        signature.push(Part::Text(": ".into()));
        signature.push(match &source.ty {
            Some(ty) => Part::Source(ty.span),
            None => Part::Type(self.properties[id].ty),
        });
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            Self::documentation_owner(self.current_owner),
            Kind::Property,
            &source.name.text,
            source.span,
            self.properties[id].access.declared,
            signature,
            true,
        );
        self.document_type_parameters(
            target,
            &source.type_params,
            &self.type_params_in_scope.clone(),
        );
        let capability = self.properties[id].capability;
        self.document_context_parameters(target, &source.context_parameters);
        if let Some(receiver) = &source.receiver_ty {
            self.document_declaration(
                Source::Receiver(hir::SourceAnnotationReceiver::Property(id)).into(),
                Some(target.into()),
                Kind::Parameter,
                "receiver",
                receiver.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(receiver.span)],
                false,
            );
        }
        let accessors = match &source.body {
            ast::PropertyBodySyntax::Initializer { accessors, .. }
            | ast::PropertyBodySyntax::Computed(accessors) => accessors,
            _ => return,
        };
        if let Some(getter) = &accessors.getter {
            self.document_declaration(
                Source::Callable(Callable::Getter(capability.getter())).into(),
                Some(target.into()),
                Kind::Getter,
                "get",
                getter.span,
                self.properties[id].access.declared,
                vec![
                    Part::Text("get(): ".into()),
                    Part::Type(self.properties[id].ty),
                ],
                true,
            );
        }
        if let Some(setter) = &accessors.setter
            && let Some(setter_id) = capability.setter()
        {
            let callable = Callable::Setter(setter_id);
            let visibility = self.property_setters[setter_id].access.declared;
            let (name, span) = match &setter.parameter {
                ast::SetterParameterSyntax::Named(parameter) => {
                    (&parameter.text[..], parameter.span)
                }
                ast::SetterParameterSyntax::Default { span } => ("value", *span),
            };
            self.document_declaration(
                Source::Callable(callable).into(),
                Some(target.into()),
                Kind::Setter,
                "set",
                setter.span,
                visibility,
                vec![
                    Part::Text(format!("set({name}: ")),
                    Part::Type(self.properties[id].ty),
                    Part::Text(")".into()),
                ],
                true,
            );
            self.document_parameter(
                callable,
                0,
                name,
                span,
                matches!(setter.parameter, ast::SetterParameterSyntax::Named(_)),
            );
        }
    }

    pub(crate) fn document_alias(
        &mut self,
        id: hir::ExportTypeAliasId,
        parameters: &[ast::TypeParamDecl],
    ) {
        if self.documentation.is_none() || self.type_aliases[id].lexical {
            return;
        }
        let alias = self.type_aliases[id].clone();
        let target = Source::TypeAlias(id);
        self.document_declaration(
            target.into(),
            alias.owner.map(|owner| Source::Nominal(owner).into()),
            Kind::TypeAlias,
            &alias.name,
            alias.origin.span,
            alias.access.declared,
            vec![Part::Source(alias.origin.span)],
            true,
        );
        self.document_type_parameters(
            target,
            parameters,
            &alias.type_params[alias.owner_parameter_count..],
        );
    }

    pub(crate) fn document_annotation(&mut self, id: scoop_identity::PersistentAnnotationId) {
        if self.documentation.is_none() {
            return;
        }
        let input = self.source_annotations[&id].clone();
        let source = &input.declaration;
        let target = Source::Annotation(id);
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::list(
            &mut signature,
            "(",
            ")",
            source.parameters.iter().map(|parameter| parameter.span),
        );
        self.document_declaration(
            target.into(),
            Self::documentation_owner(input.owner),
            Kind::Annotation,
            &source.name.text,
            source.span,
            input.access.declared,
            signature,
            true,
        );
        for (index, parameter) in source.parameters.iter().enumerate() {
            self.document_declaration(
                Target::AnnotationParameter {
                    annotation: id,
                    index: index as u32,
                },
                Some(target.into()),
                Kind::Parameter,
                &parameter.name.text,
                parameter.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(parameter.span)],
                true,
            );
        }
    }
}
