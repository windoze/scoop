use super::*;
use crate::properties::PropertyCallReceiver;

impl Lowerer {
    pub(super) fn imported_member_place(
        &mut self,
        property: crate::expr::ResolvedImportedMemberProperty,
        receiver: hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
    ) -> Option<ResolvedPlacePlan> {
        let ty = property.value_type;
        let read = self.emit_imported_member_property_read(&property, receiver.clone(), span)?;
        let write = if property.has_setter() {
            WriteCapability::ImportedMemberProperty {
                property: Box::new(property),
                receiver,
                name: name.clone(),
            }
        } else {
            WriteCapability::ReadOnly
        };
        Some(ResolvedPlacePlan { read, write, ty })
    }

    pub(super) fn resolve_imported_property_place(
        &mut self,
        binding: hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedPlacePlan> {
        let receiver = self
            .implicit_imported_property_receiver(&binding, name.span)?
            .map(|receiver| PropertyCallReceiver {
                value: self.materialize_place_expr(
                    receiver.value,
                    "property_receiver",
                    name.span,
                    sink,
                ),
                static_type: receiver.static_type,
            });
        let property =
            self.lower_imported_dependency_property_read(&binding, receiver.clone(), name.span)?;
        Some(ResolvedPlacePlan {
            ty: property.expression.ty,
            read: property.expression,
            write: if property.has_setter {
                WriteCapability::ImportedDependencyProperty {
                    binding,
                    receiver,
                    name: name.clone(),
                }
            } else {
                WriteCapability::ReadOnly
            },
        })
    }
}
