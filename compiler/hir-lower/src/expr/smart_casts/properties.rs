use super::*;
use crate::expr::imported_properties::ResolvedImportedMemberProperty;

enum MemberProperty {
    Current(hir::PropertyId, hir::MethodOwnerApplication, TypeId),
    Field(hir::FieldRef, TypeId),
    Dependency(Box<ResolvedImportedMemberProperty>),
}

impl MemberProperty {
    fn same_declaration(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Current(a, ao, _), Self::Current(b, bo, _)) => a == b && ao == bo,
            (Self::Field(a, _), Self::Field(b, _)) => a == b,
            (Self::Dependency(a), Self::Dependency(b)) => a.same_declaration(b),
            _ => false,
        }
    }
}

impl Lowerer {
    fn select_member_property_view(
        &mut self,
        views: Vec<hir::Expr>,
        name: &ast::Ident,
    ) -> Result<Option<(MemberProperty, hir::Expr)>, ()> {
        let mut candidates: Vec<(MemberProperty, hir::Expr)> = Vec::new();
        for view in views {
            let property = if let Some((property, owner, ty)) =
                self.find_accessible_nominal_property(view.ty, &name.text)
            {
                MemberProperty::Current(property, owner, ty)
            } else if let Some(field) = self.struct_field(view.ty, &name.text) {
                MemberProperty::Field(field.reference, field.ty)
            } else if let Some(property) = self.resolve_imported_member_property(view.ty, name)? {
                MemberProperty::Dependency(Box::new(property))
            } else {
                continue;
            };
            if !candidates
                .iter()
                .any(|(previous, _)| previous.same_declaration(&property))
            {
                candidates.push((property, view));
            }
        }
        if candidates.len() > 1 {
            self.error(
                name.span,
                format!("ambiguous member property `{}`", name.text),
            );
            return Err(());
        }
        Ok(candidates.pop())
    }

    pub(crate) fn smart_cast_property_receiver(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Result<hir::Expr, ()> {
        let views = self.smart_cast_receiver_views(&receiver);
        if views.len() == 1 {
            return Ok(receiver);
        }
        Ok(self
            .select_member_property_view(views, name)?
            .map_or(receiver, |(_, view)| view))
    }

    pub(crate) fn read_member_property_views(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let views = self.smart_cast_receiver_views(&receiver);
        let Some((property, receiver)) = self.select_member_property_view(views, name)? else {
            return Ok(None);
        };
        let read = match property {
            MemberProperty::Current(property, owner, ty) => {
                self.lower_property_read(property, Some(owner), Some(receiver), ty, span)
            }
            MemberProperty::Field(field, ty) => Some(hir::Expr {
                kind: ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                },
                ty,
                span,
                origin: self.expression_origin(span),
            }),
            MemberProperty::Dependency(property) => {
                if let Some(expected) = expected
                    && !self.is_subtype(property.value_type, expected)
                {
                    self.error(
                        span,
                        format!(
                            "dependency property `{}` has type {}, expected {}",
                            name.text,
                            self.type_name(property.value_type),
                            self.type_name(expected)
                        ),
                    );
                    return Err(());
                }
                self.emit_imported_member_property_read(&property, receiver, span)
            }
        };
        read.map(Some).ok_or(())
    }
}
