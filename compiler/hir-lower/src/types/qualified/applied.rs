use super::*;

impl Lowerer {
    pub(crate) fn resolve_applied_member_type(
        &mut self,
        host: TypeId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        let owner = self
            .nominal_target_for_type(host)
            .map(|target| self.nominal_identity(target.owner()).declaration_id())
            .or_else(|| self.imported_nominal_owner(host));
        let Some(owner) = owner else {
            self.error(
                span,
                format!(
                    "type `{}` cannot qualify a nested declaration",
                    self.type_name(host)
                ),
            );
            return None;
        };
        let Some(nested) = self
            .resolve_nested_type_name(owner, name, !arguments.is_empty())
            .ok()?
        else {
            self.error(
                name.span,
                format!(
                    "type `{}` has no accessible nested type `{}`",
                    self.type_name(host),
                    name.text
                ),
            );
            return None;
        };
        let nested = match nested {
            ResolvedTypeName::Nominal(nominal) => nominal,
            ResolvedTypeName::Applied(ty) => return Some(ty),
            ResolvedTypeName::Annotation(_) => {
                self.error(
                    span,
                    "annotation declarations cannot be used as value types".into(),
                );
                return None;
            }
        };
        let local = self.current_nominal_name_target(nested);
        if !self.nominal_is_companion(nested) {
            return match local {
                Some(target) => {
                    self.resolve_nested_nominal_application(target, arguments, span, &name.text)
                }
                None => self.resolve_imported_nominal_owner_arguments(nested, name, arguments),
            };
        }
        if !arguments.is_empty() {
            self.error(
                name.span,
                "companion type arguments belong to its host".into(),
            );
            return None;
        }
        self.apply_companion_type(host, nested, span)
    }

    pub(crate) fn nominal_is_companion(&self, nominal: hir::SourceNominalId) -> bool {
        match self.current_nominal_name_target(nominal) {
            Some(NominalTarget::Object(object)) => self.companion_host(object).is_some(),
            Some(_) => false,
            None => self.dependencies.as_ref().and_then(|dependencies| {
                dependencies.nominal_declaration(nominal)
            }).is_some_and(|declaration| {
                matches!(declaration.interface.source_shape(), hir::NominalSourceShapeV1::Object(shape) if shape.object_kind() == hir::ObjectSourceKindV1::Companion)
            }),
        }
    }

    pub(crate) fn apply_companion_type(
        &mut self,
        host: TypeId,
        companion: hir::SourceNominalId,
        span: ast::Span,
    ) -> Option<TypeId> {
        let arguments = self
            .nominal_application(host)
            .map_or_else(Vec::new, |application| application.arguments);
        let ty = self
            .apply_nominal_type(companion, arguments)
            .map_err(|error| {
                self.error(span, format!("cannot resolve companion type: {error:?}"));
            })
            .ok()?;
        if !self.nominal_is_accessible(ty) {
            self.error(
                span,
                format!(
                    "type `{}` is not accessible from this source location",
                    self.type_name(ty)
                ),
            );
            return None;
        }
        Some(ty)
    }
}
