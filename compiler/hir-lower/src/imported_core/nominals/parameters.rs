//! Original declaration binders for closed inheritance-family substitution.

use super::*;

impl Lowerer {
    pub(crate) fn nominal_declaration_parameters(
        &mut self,
        owner: hir::SourceNominalId,
    ) -> Result<Vec<hir::TypeParamDecl>, ImportedSignatureTypeError> {
        if let Some(owner) = self.nominal_owners.get(&owner).copied() {
            return Ok(self.owner_type_params(owner));
        }
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        Ok(match declaration.interface.kind() {
            hir::PublicNominalKindV1::Class | hir::PublicNominalKindV1::Object => {
                if !self.loaded_class_definitions.contains_key(&owner) {
                    self.load_class_definition(declaration)?;
                }
                self.class_definition(owner).type_params.clone()
            }
            hir::PublicNominalKindV1::Interface => {
                if !self.loaded_interface_definitions.contains_key(&owner) {
                    self.load_interface_definition(declaration)?;
                }
                self.interface_definition(owner).type_params.clone()
            }
            hir::PublicNominalKindV1::Struct => {
                if !self.loaded_struct_definitions.contains_key(&owner) {
                    self.load_struct_definition(declaration)?;
                }
                self.struct_definition(owner).type_params.clone()
            }
            hir::PublicNominalKindV1::Enum => {
                if !self.loaded_enum_definitions.contains_key(&owner) {
                    self.load_enum_definition(declaration)?;
                }
                self.enum_definition(owner).type_params.clone()
            }
        })
    }
}
