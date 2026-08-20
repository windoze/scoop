use super::*;

impl Lowerer {
    pub(crate) fn apply_inner_type(
        &mut self,
        outer: TypeId,
        inner: hir::SourceNominalId,
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        let host = self.nominal_application(outer)?;
        if self.nominal_enclosing_owner(inner) != Some(host.template) {
            self.error(
                span,
                "inner type requires its declaring outer application".into(),
            );
            return None;
        }
        let arity = match self.nominal_owners.get(&inner).copied() {
            Some(crate::Owner::Class(class)) => self.classes[class].type_params.len(),
            Some(_) => unreachable!("only a class can have an enclosing instance"),
            None => self
                .dependencies
                .as_ref()?
                .nominal_declaration(inner)?
                .interface
                .type_parameters()
                .binders()
                .len(),
        };
        let own_arity = arity - host.arguments.len();
        if own_arity != arguments.len() {
            self.error(
                span,
                format!(
                    "inner class takes {own_arity} type argument(s), but {} were supplied",
                    arguments.len(),
                ),
            );
            return None;
        }
        let mut resolved = host.arguments;
        for argument in arguments {
            resolved.push(self.resolve_type_ref(argument)?);
        }
        let ty = self
            .apply_nominal_type(inner, resolved.clone())
            .map_err(|error| self.error(span, format!("cannot resolve inner type: {error:?}")))
            .ok()?;
        let parameters = self.class_definition(inner).type_params.clone();
        if !self.nominal_is_accessible(ty) {
            self.error(span, "inner class is not accessible here".into());
            return None;
        }
        self.check_type_argument_kinds(&parameters, &resolved, span, "inner class")
            .then_some(ty)
    }
}
