//! Source declarations behind imported primitive representations.

use super::*;

impl Lowerer {
    pub(crate) fn retain_imported_box_source(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        let kind = match self.types[ty] {
            hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
            hir::Type::Boolean => hir::IntrinsicTypeKind::Boolean,
            _ => return Ok(()),
        };
        self.resolve_imported_intrinsic_type(kind)
    }

    pub(crate) fn resolve_imported_intrinsic_type(
        &mut self,
        kind: hir::IntrinsicTypeKind,
    ) -> Result<(), ImportedSignatureTypeError> {
        if self.imported_intrinsic_types.contains_key(&kind) {
            return Ok(());
        }
        let CoreLoweringAuthority::Imported(protocols) = &self.core else {
            return Ok(());
        };
        let fundamental = protocols.fundamental_types();
        let identity = match kind {
            hir::IntrinsicTypeKind::Integer(kind) => fundamental.integer(kind).persistent(),
            hir::IntrinsicTypeKind::Boolean => fundamental.boolean().persistent(),
            hir::IntrinsicTypeKind::String => fundamental.string().persistent(),
            hir::IntrinsicTypeKind::Array
            | hir::IntrinsicTypeKind::MutableArray
            | hir::IntrinsicTypeKind::Ptr
            | hir::IntrinsicTypeKind::FunPtr => return Err(ImportedSignatureTypeError::Generic),
        };
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal(identity))
            .cloned()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        let interfaces = declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type(parent))
            .collect::<Result<Vec<_>, _>>()?;
        self.imported_intrinsic_types.insert(
            kind,
            hir::ImportedIntrinsicType {
                declaration,
                interfaces,
            },
        );
        Ok(())
    }
}

impl ImportedSignatureTypeError {
    pub(crate) fn diagnostic(self, subject: &str) -> String {
        match self {
            Self::Generic => crate::imported_capabilities::ImportedCapabilityRequirement::Generic
                .diagnostic(subject),
            Self::Structural => {
                format!("cannot resolve {subject} from dependency declarations")
            }
        }
    }
}
