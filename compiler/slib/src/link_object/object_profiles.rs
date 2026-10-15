//! Producer profiles over Mach-O and ELF object facts.

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
    pub(in crate::link_object) fn after_digest_patch(
        mut sections: ValidatedBuiltinObjectSectionInventoryV1,
        bytes: &[u8],
    ) -> Self {
        // Digest slots cannot alter the parsed section, symbol or relocation structure.
        sections.envelope.content_digest = scoop_wire::sha256(bytes);
        Self { sections }
    }

    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }

    pub fn into_sections(self) -> ValidatedBuiltinObjectSectionInventoryV1 {
        self.sections
    }
}

pub fn validate_scoop_lir_llvm_22_1_object_envelope_v1(
    target: scoop_lir::LirTargetProfile,
    bytes: &[u8],
) -> Result<ValidatedScoopLirObjectEnvelopeV1, ScoopLirObjectEnvelopeValidationError> {
    if scoop_target_data::layout(target.id()).map(|profile| profile.native_object_format())
        != Some(target.native_object_format())
    {
        return Err(ScoopLirObjectEnvelopeValidationError::UnsupportedTarget);
    }
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        return crate::link_object::elf::builtin::sections(
            bytes,
            target.id(),
            BuiltinLinkObjectSectionProfileV1::ScoopLir,
        )
        .map(|sections| ValidatedScoopLirObjectEnvelopeV1 { sections })
        .map_err(ScoopLirObjectEnvelopeValidationError::Elf);
    }
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

pub fn validate_generated_c_object_for_profile_v1(
    bytes: &[u8],
    profile: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<ValidatedGeneratedCBridgeObjectEnvelopeV1, GeneratedCBridgeObjectEnvelopeValidationError>
{
    if scoop_target_data::c_bridge_toolchain_id(profile.contract().target().id()).as_ref()
        != Some(profile.id())
    {
        return Err(GeneratedCBridgeObjectEnvelopeValidationError::UnsupportedTarget);
    }
    match (
        profile.contract().platform(),
        profile.contract().target().id(),
    ) {
        (
            scoop_lir::CBridgePlatformContractV1::Darwin { deployment, .. },
            scoop_target_data::DARWIN_AARCH64_ID,
        ) => validate_generated_c_bridge_object_envelope_v1(bytes, deployment),
        (
            scoop_lir::CBridgePlatformContractV1::Linux { .. },
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID,
        ) => crate::link_object::elf::builtin::sections(
            bytes,
            profile.contract().target().id(),
            BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
        )
        .map(|sections| ValidatedGeneratedCBridgeObjectEnvelopeV1 { sections })
        .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Elf),
        _ => Err(GeneratedCBridgeObjectEnvelopeValidationError::UnsupportedTarget),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopLirObjectEnvelopeValidationError {
    UnsupportedTarget,
    Elf(crate::link_object::ElfObjectError),
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
    UnsupportedTarget,
    Elf(crate::link_object::ElfObjectError),
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
