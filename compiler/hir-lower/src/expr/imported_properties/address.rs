use super::*;
use scoop_identity::{PersistentPropertyId, SourceNativeExternalContract};

impl Lowerer {
    pub(super) fn imported_property_native_place(
        &mut self,
        property: &hir::ImportedDependencyPropertyCandidate,
        span: ast::Span,
    ) -> Option<Option<hir::Place>> {
        let scoop_identity::PropertyOwner::Property(property) = property.interface().declaration()
        else {
            return Some(None);
        };
        if self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_native_contract(property))
            .is_none()
        {
            return Some(None);
        }
        match self.external_global_place(property) {
            Ok(place) => Some(Some(place)),
            Err(message) => {
                self.error(span, message);
                None
            }
        }
    }

    pub(super) fn external_storage_pointer(
        &mut self,
        place: hir::Place,
        span: ast::Span,
    ) -> hir::Expr {
        let hir::Place::ExternalGlobal { ty, .. } = &place else {
            unreachable!("an imported native property retains its external storage place")
        };
        let ty = self.intern_type(hir::Type::Ptr(*ty));
        hir::Expr {
            kind: hir::ExprKind::AddressOf(place),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(in crate::expr) fn external_global_place(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<hir::Place, String> {
        let source_contract = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_native_contract(property))
            .ok_or_else(|| format!("property {property} has no extern storage contract"))?;
        let storage = match source_contract.contract() {
            SourceNativeExternalContract::ReadOnlyData { storage, .. }
            | SourceNativeExternalContract::MutableData { storage, .. }
            | SourceNativeExternalContract::ReadOnlyTls { storage, .. }
            | SourceNativeExternalContract::MutableTls { storage, .. } => storage,
            SourceNativeExternalContract::Function { .. } => {
                return Err("property has a function contract".into());
            }
        };
        let ty = self
            .imported_signature_type(storage)
            .map_err(|error| format!("invalid extern property type: {error:?}"))?;
        Ok(hir::Place::ExternalGlobal {
            property,
            source_contract,
            ty,
        })
    }
}
