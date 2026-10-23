use super::*;

impl Lowerer {
    pub(crate) fn substitute_companion_type(&mut self, host: TypeId) -> TypeId {
        // Alpha-renaming need not install the destination binder's scope.
        if let Type::Param(parameter) = self.types[host] {
            return self.intern_type(Type::Companion(parameter));
        }
        self.projected_companion_type(host)
            .expect("applicable substitutions satisfy companion conditions")
    }

    /// Resolve the declared companion without evaluating its singleton.
    pub(crate) fn projected_companion_type(&mut self, host: TypeId) -> Result<TypeId, String> {
        if let Type::Param(parameter) = self.types[host] {
            if self
                .type_params_in_scope
                .iter()
                .any(|p| p.id == parameter && p.companion.is_some())
            {
                return Ok(self.intern_type(Type::Companion(parameter)));
            }
            return Err(format!(
                "the generic environment does not require a public companion for `{}`",
                self.type_name(host)
            ));
        }
        let companion = self.public_companion_declaration(host);
        let companion = companion.ok_or_else(|| {
            format!(
                "static type `{}` has no public companion",
                self.type_name(host)
            )
        })?;
        let arguments = self
            .nominal_application(host)
            .map_or_else(Vec::new, |application| application.arguments);
        let ty = self
            .apply_nominal_type(companion, arguments)
            .map_err(|error| format!("cannot apply companion: {error:?}"))?;
        // An open body can evaluate this projection only after concretization.
        // Retain the existing generic singleton template for that substitution;
        // loading it does not execute its initialization unit.
        if let hir::SourceNominalId::GenericTemplate(owner) = companion
            && self
                .dependencies
                .as_ref()
                .is_some_and(|dependencies| dependencies.nominal_declaration(companion).is_some())
        {
            self.request_imported_companion(owner)?;
        }
        Ok(ty)
    }

    pub(crate) fn public_companion_declaration(
        &self,
        host: TypeId,
    ) -> Option<hir::SourceNominalId> {
        if let Some(target) = self.nominal_target_for_type(host) {
            let object = self.companion_object(target.owner());
            object
                .filter(|object| {
                    self.objects[*object].access.declared == hir::DeclaredVisibility::Public
                })
                .map(|object| {
                    self.nominal_identity(crate::Owner::Object(object))
                        .declaration_id()
                })
        } else if let Some(owner) = self.imported_nominal_owner(host) {
            self.dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(owner))
                .map(|declaration| {
                    declaration
                        .interface
                        .declaration_details()
                        .children()
                        .values()
                        .to_vec()
                })
                .unwrap_or_default()
                .into_iter()
                .find(|child| {
                    self.nominal_is_companion(*child)
                        && self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| dependencies.nominal_declaration(*child))
                            .is_some_and(|declaration| {
                                declaration
                                    .interface
                                    .declaration_details()
                                    .declared_visibility()
                                    == hir::DeclaredVisibilityV1::Public
                            })
                })
        } else {
            None
        }
    }

    pub(super) fn resolve_parameter_companion(
        &mut self,
        parameter: hir::TypeParamId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
    ) -> Option<TypeId> {
        if name.text != "Companion" {
            self.error(
                name.span,
                "a dependent type only supports the `Companion` projection".into(),
            );
            return None;
        }
        if !arguments.is_empty() {
            self.error(
                name.span,
                "companion type arguments belong to its host".into(),
            );
            return None;
        }
        let host = self.intern_type(Type::Param(parameter));
        self.projected_companion_type(host)
            .map_err(|message| self.error(name.span, message))
            .ok()
    }

    pub(crate) fn parameter_nominal_bounds(&self, ty: TypeId) -> Vec<hir::NominalBoundRef<'_>> {
        let (parameter, companion) = match self.types[ty] {
            Type::Param(parameter) => (parameter, false),
            Type::Companion(parameter) => (parameter, true),
            _ => return Vec::new(),
        };
        let Some(parameter) = self.type_params_in_scope.iter().find(|p| p.id == parameter) else {
            return Vec::new();
        };
        let bounds = if companion {
            let Some(companion) = &parameter.companion else {
                return Vec::new();
            };
            &companion.bounds
        } else {
            &parameter.bounds
        };
        match bounds {
            hir::TypeParamBounds::Nominal(bounds) => bounds.in_source_order(),
            _ => Vec::new(),
        }
    }
}
