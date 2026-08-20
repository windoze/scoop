//! Char keeps its source nominal identity in the ordinary struct type channel.

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use crate::{CoreLoweringAuthority, IntrinsicTypeOwner};

impl Lowerer {
    pub(crate) fn core_character_type(&mut self) -> Result<TypeId, ImportedSignatureTypeError> {
        match &self.core {
            CoreLoweringAuthority::Defined => {
                let Some(&(IntrinsicTypeOwner::Struct(owner), _)) = self
                    .intrinsic_type_owners
                    .get(&hir::IntrinsicTypeKind::Char)
                else {
                    return Err(ImportedSignatureTypeError::Structural);
                };
                Ok(self.struct_applications[self.structs[owner].self_application].canonical_type)
            }
            CoreLoweringAuthority::Imported(core) => {
                let owner = core.fundamental_types().character().persistent();
                self.imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            }
        }
    }

    pub(crate) fn is_char_type(&self, ty: TypeId) -> bool {
        let Type::Struct(application) = self.types[ty] else {
            return false;
        };
        matches!(
            self.struct_definition(self.struct_applications[application].template)
                .representation,
            hir::StructRepresentation::Intrinsic(hir::IntrinsicTypeKind::Char)
        )
    }
}
