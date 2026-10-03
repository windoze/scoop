//! Producer-specific qualification layered on the shared Mach-O envelope.

use std::fmt;

use scoop_lir::DarwinCBridgeDeploymentContractV1;

use super::{
    BuiltinLinkObjectSectionProfileV1, BuiltinObjectSectionValidationError,
    DarwinBuildToolVersionV1, DarwinDeploymentCommandV1, ObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, validate_builtin_object_section_inventory_v1,
    validate_darwin_arm64_object_envelope_v1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedScoopLirObjectEnvelopeV1 {
    sections: ValidatedBuiltinObjectSectionInventoryV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedGeneratedCBridgeObjectEnvelopeV1 {
    sections: ValidatedBuiltinObjectSectionInventoryV1,
}

impl ValidatedGeneratedCBridgeObjectEnvelopeV1 {
    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }

    pub fn into_sections(self) -> ValidatedBuiltinObjectSectionInventoryV1 {
        self.sections
    }
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

pub fn validate_generated_c_bridge_object_envelope_v1(
    bytes: &[u8],
    deployment: &DarwinCBridgeDeploymentContractV1,
) -> Result<ValidatedGeneratedCBridgeObjectEnvelopeV1, GeneratedCBridgeObjectEnvelopeValidationError>
{
    let envelope = validate_darwin_arm64_object_envelope_v1(bytes)
        .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Envelope)?;
    let Some(actual) = envelope.deployment() else {
        return Err(GeneratedCBridgeObjectEnvelopeValidationError::MissingDeployment);
    };
    let expected = DarwinDeploymentCommandV1::BuildVersion {
        minimum_os: deployment.minimum_os().packed(),
        sdk: deployment.sdk().packed(),
        tools: deployment
            .tools()
            .iter()
            .map(|tool| {
                DarwinBuildToolVersionV1::new(tool.tool().macho_value(), tool.version().packed())
            })
            .collect(),
    };
    if actual != &expected {
        return Err(
            GeneratedCBridgeObjectEnvelopeValidationError::DeploymentMismatch {
                expected,
                actual: actual.clone(),
            },
        );
    }
    let sections = validate_builtin_object_section_inventory_v1(
        envelope,
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    )
    .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Sections)?;
    Ok(ValidatedGeneratedCBridgeObjectEnvelopeV1 { sections })
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedCBridgeObjectEnvelopeValidationError {
    Envelope(ObjectEnvelopeValidationError),
    MissingDeployment,
    DeploymentMismatch {
        expected: DarwinDeploymentCommandV1,
        actual: DarwinDeploymentCommandV1,
    },
    Sections(BuiltinObjectSectionValidationError),
}

impl fmt::Display for GeneratedCBridgeObjectEnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid generated C bridge object envelope: {self:?}"
        )
    }
}

impl std::error::Error for GeneratedCBridgeObjectEnvelopeValidationError {}

#[cfg(test)]
mod tests;
