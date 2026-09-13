//! Producer-specific qualification layered on the shared Mach-O envelope.

use std::fmt;

use super::{
    BuiltinLinkObjectSectionProfileV1, BuiltinObjectSectionValidationError,
    DarwinDeploymentCommandV1, ObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, validate_builtin_object_section_inventory_v1,
    validate_darwin_arm64_object_envelope_v1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedScoopLirObjectEnvelopeV1 {
    sections: ValidatedBuiltinObjectSectionInventoryV1,
}

impl ValidatedScoopLirObjectEnvelopeV1 {
    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }

    pub fn into_sections(self) -> ValidatedBuiltinObjectSectionInventoryV1 {
        self.sections
    }
}

pub fn validate_scoop_lir_llvm_22_1_object_envelope_v1(
    bytes: &[u8],
) -> Result<ValidatedScoopLirObjectEnvelopeV1, ScoopLirObjectEnvelopeValidationError> {
    let envelope = validate_darwin_arm64_object_envelope_v1(bytes)
        .map_err(ScoopLirObjectEnvelopeValidationError::Envelope)?;
    if let Some(actual) = envelope.deployment() {
        return Err(ScoopLirObjectEnvelopeValidationError::UnexpectedDeployment(
            actual.clone(),
        ));
    }
    let sections = validate_builtin_object_section_inventory_v1(
        envelope,
        BuiltinLinkObjectSectionProfileV1::ScoopLir,
    )
    .map_err(ScoopLirObjectEnvelopeValidationError::Sections)?;
    Ok(ValidatedScoopLirObjectEnvelopeV1 { sections })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopLirObjectEnvelopeValidationError {
    Envelope(ObjectEnvelopeValidationError),
    UnexpectedDeployment(DarwinDeploymentCommandV1),
    Sections(BuiltinObjectSectionValidationError),
}

impl fmt::Display for ScoopLirObjectEnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid LLVM 22.1 Scoop LIR object envelope: {self:?}"
        )
    }
}

impl std::error::Error for ScoopLirObjectEnvelopeValidationError {}

#[cfg(test)]
mod tests;
