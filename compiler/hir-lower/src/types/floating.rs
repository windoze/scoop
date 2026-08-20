//! Floating scalars retain the actual intrinsic nominal declaration.

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use crate::{CoreLoweringAuthority, IntrinsicTypeOwner};

impl Lowerer {
    pub(crate) fn core_float_type(
        &mut self,
        kind: hir::FloatKind,
    ) -> Result<TypeId, ImportedSignatureTypeError> {
        match &self.core {
            CoreLoweringAuthority::Defined => {
                let Some(&(IntrinsicTypeOwner::Struct(owner), _)) = self
                    .intrinsic_type_owners
                    .get(&hir::IntrinsicTypeKind::Float(kind))
                else {
                    return Err(ImportedSignatureTypeError::Structural);
                };
                Ok(self.struct_applications[self.structs[owner].self_application].canonical_type)
            }
            CoreLoweringAuthority::Imported(core) => {
                let owner = core.fundamental_types().floating(kind).persistent();
                self.imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            }
        }
    }

    pub(crate) fn float_kind(&self, ty: TypeId) -> Option<hir::FloatKind> {
        let Type::Struct(application) = self.types[ty] else {
            return None;
        };
        match self
            .struct_definition(self.struct_applications[application].template)
            .representation
        {
            hir::StructRepresentation::Intrinsic(hir::IntrinsicTypeKind::Float(kind)) => Some(kind),
            _ => None,
        }
    }
}
