use super::*;

impl Lowerer {
    pub(crate) fn document_function(&mut self, id: hir::FunctionId, source: &ast::FunctionDecl) {
        if self.documentation.is_none() || !self.source_function_declarations.contains_key(&id) {
            return;
        }
        let callable = Callable::Function(id);
        let target = Source::Callable(callable);
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::list(
            &mut signature,
            "(",
            ")",
            source.params.iter().map(|param| param.span),
        );
        signature.push(Part::Text(": ".into()));
        signature.push(match &source.return_ty {
            Some(ty) => Part::Source(ty.span),
            None => Part::Type(self.signatures[&id].return_ty),
        });
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            Self::documentation_owner(self.current_owner),
            Kind::Function,
            &source.name.text,
            source.span,
            self.functions[id].access.declared,
            signature,
            true,
        );
        let parameters = self.signatures[&id].type_params
            [self.signatures[&id].owner_type_param_count..]
            .to_vec();
        self.document_type_parameters(target, &source.type_params, &parameters);
        for (index, parameter) in source.params.iter().enumerate() {
            self.document_parameter(callable, index, &parameter.name.text, parameter.span, true);
        }
        self.document_context_parameters(target, &source.context_parameters);
        if let Some(receiver) = &source.receiver_ty {
            self.document_declaration(
                Source::Receiver(hir::SourceAnnotationReceiver::Callable(callable)).into(),
                Some(target.into()),
                Kind::Parameter,
                "receiver",
                receiver.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(receiver.span)],
                false,
            );
        }
    }

    pub(super) fn document_context_parameters(
        &mut self,
        owner: Source,
        parameters: &[ast::ContextParameter],
    ) {
        for (index, parameter) in parameters.iter().enumerate() {
            let name = match &parameter.label {
                ast::ContextParameterLabel::Named(name) => name.text.as_str(),
                ast::ContextParameterLabel::Unnamed(_) => "_",
            };
            self.document_declaration(
                Target::ContextParameter {
                    owner,
                    index: index as u32,
                },
                Some(owner.into()),
                Kind::ContextParameter,
                name,
                parameter.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(parameter.span)],
                true,
            );
        }
    }
}
