//! Singleton reads retain their provider's object value identity.

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use scoop_identity::{PersistentObjectValueId, SignatureTypeKey};

impl Lowerer {
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
            .id();
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
        Some(hir::Expr {
            kind: ExprKind::ImportedSingletonValue(value),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
