//! Producer-specific qualification layered on the shared Mach-O envelope.

use std::fmt;

use super::{
    BuiltinLinkObjectSectionProfileV1, BuiltinObjectSectionValidationError,
    DarwinBuildToolVersionV1, DarwinDeploymentCommandV1, ObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, validate_builtin_object_section_inventory_v1,
    validate_darwin_arm64_object_envelope_v1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DarwinGeneratedCDeploymentContractV1 {
    minimum_os: u32,
    sdk: u32,
    tools: Vec<DarwinBuildToolVersionV1>,
}

impl DarwinGeneratedCDeploymentContractV1 {
    pub fn new(
        minimum_os: u32,
        sdk: u32,
        tools: Vec<DarwinBuildToolVersionV1>,
    ) -> Result<Self, DarwinGeneratedCDeploymentContractError> {
        if minimum_os == 0 {
            return Err(DarwinGeneratedCDeploymentContractError::ZeroMinimumOs);
        }
        if sdk == 0 {
            return Err(DarwinGeneratedCDeploymentContractError::ZeroSdk);
        }
        for (index, tool) in tools.iter().enumerate() {
            if tool.tool() == 0 {
                return Err(DarwinGeneratedCDeploymentContractError::ZeroTool { index });
            }
            if tool.version() == 0 {
                return Err(DarwinGeneratedCDeploymentContractError::ZeroToolVersion { index });
            }
            if index > 0 && tools[index - 1].tool() >= tool.tool() {
                return Err(if tools[index - 1].tool() == tool.tool() {
                    DarwinGeneratedCDeploymentContractError::DuplicateTool(tool.tool())
                } else {
                    DarwinGeneratedCDeploymentContractError::NonCanonicalToolOrder { index }
                });
            }
        }
        Ok(Self {
            minimum_os,
            sdk,
            tools,
        })
    }

    pub const fn minimum_os(&self) -> u32 {
        self.minimum_os
    }

    pub const fn sdk(&self) -> u32 {
        self.sdk
    }

    pub fn tools(&self) -> &[DarwinBuildToolVersionV1] {
        &self.tools
    }

    fn deployment(&self) -> DarwinDeploymentCommandV1 {
        DarwinDeploymentCommandV1::BuildVersion {
            minimum_os: self.minimum_os,
            sdk: self.sdk,
            tools: self.tools.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinGeneratedCDeploymentContractError {
    ZeroMinimumOs,
    ZeroSdk,
    ZeroTool { index: usize },
    ZeroToolVersion { index: usize },
    DuplicateTool(u32),
    NonCanonicalToolOrder { index: usize },
}

impl fmt::Display for DarwinGeneratedCDeploymentContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin generated-C deployment contract: {self:?}"
        )
    }
}

impl std::error::Error for DarwinGeneratedCDeploymentContractError {}

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
    deployment: &DarwinGeneratedCDeploymentContractV1,
) -> Result<ValidatedGeneratedCBridgeObjectEnvelopeV1, GeneratedCBridgeObjectEnvelopeValidationError>
{
    let envelope = validate_darwin_arm64_object_envelope_v1(bytes)
        .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Envelope)?;
    let Some(actual) = envelope.deployment() else {
        return Err(GeneratedCBridgeObjectEnvelopeValidationError::MissingDeployment);
    };
    let expected = deployment.deployment();
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
