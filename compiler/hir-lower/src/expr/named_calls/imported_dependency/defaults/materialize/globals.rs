use super::*;
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyId, PropertyOwner};

impl Lowerer {
    pub(super) fn materialize_imported_global_read(
        &mut self,
        property: PersistentPropertyId,
        ty: hir::TypeId,
        span: Span,
        origin: hir::ExpressionOrigin,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        if self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_native_contract(property))
            .is_some()
        {
            let place = self
                .external_global_place(property)
                .map_err(ImportedDefaultMaterializationError::Plan)?;
            let pointer = hir::Expr {
                kind: hir::ExprKind::AddressOf(place),
                ty: self.intern_type(hir::Type::Ptr(ty)),
                span,
                origin,
            };
            return Ok(hir::ExprKind::PtrLoad {
                pointer: Box::new(pointer),
                offset: None,
            });
        }
        let getter = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| {
                dependencies.property_declaration(PropertyOwner::Property(property))
            })
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(format!(
                    "dependency default global {property} has no property declaration"
                ))
            })?
            .accessors()
            .getter();
        self.imported_default_call_kind(
            CallableTemplateOrigin::Accessor(getter),
            Vec::new(),
            hir::SourceCallReceiver::NoReceiver,
            MemberCallKind::Ordinary,
        )
    }
}
