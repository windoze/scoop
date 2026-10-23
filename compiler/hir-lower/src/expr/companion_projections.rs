use super::*;

impl Lowerer {
    pub(crate) fn companion_singleton_target(
        &mut self,
        ty: TypeId,
    ) -> Result<hir::SingletonValueTarget, String> {
        if let Type::Companion(parameter) = self.types[ty] {
            return Ok(hir::SingletonValueTarget::Companion(parameter));
        }
        if let Some(crate::NominalTarget::Object(object)) = self.nominal_target_for_type(ty) {
            return Ok(hir::SingletonValueTarget::Local(
                self.objects[object].singleton_value,
            ));
        }
        let owner = self
            .imported_nominal_owner(ty)
            .ok_or("companion owner is absent")?;
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .ok_or("companion declaration is absent")?;
        let hir::NominalSourceShapeV1::Object(shape) = declaration.interface.source_shape() else {
            return Err("companion declaration is not an object".into());
        };
        let value = shape.value();
        if let hir::SourceNominalId::GenericTemplate(owner) = owner {
            self.request_imported_companion(owner)?;
        }
        Ok(hir::SingletonValueTarget::Dependency(value))
    }

    pub(in crate::expr) fn companion_parameter_qualifier(
        &self,
        expression: &ast::Expr,
    ) -> Option<hir::TypeParamId> {
        let name = match expression {
            ast::Expr::Var(name) => name,
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(member) = &access.selector else {
                    return None;
                };
                if member.text != "Companion" {
                    return None;
                }
                let ast::Expr::Var(name) = &*access.receiver else {
                    return None;
                };
                name
            }
            _ => return None,
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&name.text) {
            return None;
        }
        self.type_params_in_scope
            .iter()
            .find(|p| p.name == name.text)
            .map(|p| p.id)
    }

    pub(crate) fn lower_parameter_companion(
        &mut self,
        parameter: hir::TypeParamId,
        span: Span,
    ) -> Option<hir::Expr> {
        let host = self.intern_type(Type::Param(parameter));
        let ty = self
            .projected_companion_type(host)
            .map_err(|message| self.error(span, message))
            .ok()?;
        Some(hir::Expr {
            kind: ExprKind::SingletonValue(hir::SingletonValueTarget::Companion(parameter)),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
