//! Singleton reads retain their provider's object value identity.

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use scoop_identity::{PersistentObjectValueId, SignatureTypeKey};

impl Lowerer {
    pub(crate) fn imported_object_value(
        &self,
        owner: scoop_identity::PersistentTypeId,
    ) -> Option<PersistentObjectValueId> {
        let declaration = self.dependencies.as_ref()?.nominal(owner)?;
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Object(shape) => Some(shape.value()),
            _ => None,
        }
    }

    pub(crate) fn imported_singleton_type(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<TypeId, ImportedSignatureTypeError> {
        let owner = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.singleton_owner(value))
            .ok_or(ImportedSignatureTypeError::Structural)?
            .identity
            .concrete_id()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        self.imported_signature_type(&SignatureTypeKey::Nominal(owner))
    }

    pub(in crate::expr) fn lower_imported_singleton(
        &mut self,
        value: PersistentObjectValueId,
        span: Span,
    ) -> Option<hir::Expr> {
        let ty = match self.imported_singleton_type(value) {
            Ok(ty) => ty,
            Err(error) => {
                self.error(span, format!("invalid dependency singleton: {error:?}"));
                return None;
            }
        };
        self.lower_imported_singleton_application(value, ty, span)
    }

    pub(in crate::expr) fn lower_imported_singleton_application(
        &mut self,
        value: PersistentObjectValueId,
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        if let Some(hir::SourceNominalId::GenericTemplate(owner)) = self.imported_nominal_owner(ty)
            && let Err(error) = self.request_imported_companion(owner)
        {
            self.error(span, format!("invalid dependency companion: {error}"));
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::SingletonValue(hir::SingletonValueTarget::Dependency(value)),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(in crate::expr) fn lower_imported_singleton_type(
        &mut self,
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let owner = self.imported_nominal_owner(ty)?;
        let declaration = self.dependencies.as_ref()?.nominal_declaration(owner)?;
        let hir::NominalSourceShapeV1::Object(shape) = declaration.interface.source_shape() else {
            return None;
        };
        self.lower_imported_singleton_application(shape.value(), ty, span)
    }
}
