use super::*;

impl Lowerer {
    pub(crate) fn apply_source_type_alias(
        &mut self,
        id: SourceTypeAliasId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
    ) -> Option<hir::TypeId> {
        self.apply_source_type_alias_with_host(id, name, arguments, None)
    }

    pub(crate) fn source_alias_template(
        &mut self,
        id: SourceTypeAliasId,
        name: &ast::Ident,
    ) -> Option<AliasTemplate> {
        if !self.source_type_alias_is_accessible(id) {
            self.error(
                name.span,
                format!(
                    "typealias `{}` is not accessible from this source location",
                    name.text
                ),
            );
            return None;
        }
        let target = self.resolve_type_alias(id, name.span)?;
        let alias = &self.source_type_aliases[id];
        Some(AliasTemplate {
            origin: AliasOrigin::Source(id),
            name: name.clone(),
            host: None,
            arguments: None,
            parameters: alias.parameters.clone(),
            outer_parameter_count: alias.outer_parameter_count,
            target,
        })
    }

    pub(crate) fn apply_source_type_alias_with_host(
        &mut self,
        id: SourceTypeAliasId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        host: Option<hir::TypeId>,
    ) -> Option<hir::TypeId> {
        let template = self.source_alias_template(id, name)?;
        self.apply_alias_template(&template, name, arguments, host)
    }

    pub(crate) fn apply_alias_template(
        &mut self,
        template: &AliasTemplate,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        host: Option<hir::TypeId>,
    ) -> Option<hir::TypeId> {
        let outer_count = template.outer_parameter_count;
        let parameters = &template.parameters;
        let own_count = parameters.len() - outer_count;
        if arguments.len() != own_count {
            let message = if own_count == 0 {
                format!("typealias `{}` is not generic", name.text)
            } else {
                format!(
                    "typealias `{}` takes {own_count} type argument(s), but {} were supplied",
                    name.text,
                    arguments.len()
                )
            };
            self.error(name.span, message);
            return None;
        }
        let mut actual = self.alias_outer_arguments(template, host.or(template.host), name.span)?;
        actual.extend(
            arguments
                .iter()
                .map(|argument| self.resolve_type_ref(argument))
                .collect::<Option<Vec<_>>>()?,
        );
        self.check_type_argument_kinds(
            parameters,
            &actual,
            name.span,
            &format!("typealias `{}`", name.text),
        );
        Some(self.instantiate_ty(template.target, &actual))
    }
}

impl Lowerer {
    pub(super) fn alias_outer_arguments(
        &mut self,
        template: &AliasTemplate,
        host: Option<hir::TypeId>,
        span: ast::Span,
    ) -> Option<Vec<hir::TypeId>> {
        let count = template.outer_parameter_count;
        if count == 0 {
            return Some(Vec::new());
        }
        let actual = if let Some(application) = host.and_then(|ty| self.nominal_application(ty)) {
            application.arguments
        } else {
            if template.parameters[..count].iter().any(|parameter| {
                !self
                    .type_params_in_scope
                    .iter()
                    .any(|visible| visible.id == parameter.id)
            }) {
                self.error(
                    span,
                    format!(
                        "typealias `{}` requires a complete host type application",
                        template.name.text
                    ),
                );
                return None;
            }
            template.parameters[..count]
                .iter()
                .map(|parameter| self.intern_type(Type::Param(parameter.id)))
                .collect()
        };
        if actual.len() != count {
            self.error(
                span,
                format!(
                    "typealias `{}` requires {count} host type argument(s)",
                    template.name.text
                ),
            );
            return None;
        }
        Some(actual)
    }

    pub(crate) fn reload_alias_template(&mut self, alias: &AliasTemplate) -> Option<AliasTemplate> {
        let mut loaded = match &alias.origin {
            AliasOrigin::Source(id) => self.source_alias_template(*id, &alias.name)?,
            AliasOrigin::Imported(binding) => self.imported_alias_template(binding, &alias.name)?,
        };
        loaded.host = alias.host;
        loaded.arguments = alias.arguments.clone();
        Some(loaded)
    }
}
