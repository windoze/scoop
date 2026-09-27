use super::*;
use crate::properties::PropertyCallReceiver;

impl Lowerer {
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
