//! Retain companion declarations for generic arguments before concrete queries.

use super::*;
use crate::imported_core::ImportedSignatureTypeError;

impl Lowerer {
    pub(crate) fn conditional_interfaces_use_companions(&self) -> bool {
        self.classes
            .values()
            .flat_map(|owner| &owner.conditional_interfaces)
            .chain(
                self.structs
                    .values()
                    .flat_map(|owner| &owner.conditional_interfaces),
            )
            .chain(
                self.enums
                    .values()
                    .flat_map(|owner| &owner.conditional_interfaces),
            )
            .chain(
                self.loaded_class_definitions
                    .values()
                    .flat_map(|owner| &owner.definition.conditional_interfaces),
            )
            .chain(
                self.loaded_struct_definitions
                    .values()
                    .flat_map(|owner| &owner.definition.conditional_interfaces),
            )
            .chain(
                self.loaded_enum_definitions
                    .values()
                    .flat_map(|owner| &owner.definition.conditional_interfaces),
            )
            .any(|implementation| {
                implementation
                    .parameters
                    .iter()
                    .any(|parameter| parameter.companion.is_some())
            })
    }

    pub(crate) fn retain_conditional_companion(
        &mut self,
        argument: TypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        let Some(companion) = self.public_companion_declaration(argument) else {
            return Ok(());
        };
        if self.nominal_owners.contains_key(&companion)
            || !self
                .conditional_interfaces
                .retained_companions
                .insert(companion)
        {
            return Ok(());
        }
        self.nominal_declaration_parameters(companion)?;
        if let hir::SourceNominalId::GenericTemplate(owner) = companion {
            self.request_imported_companion(owner)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
        }
        Ok(())
    }
}
