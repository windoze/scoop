//! Read target object facts used by object production.

use std::fmt;

use super::{
    ObjectEnvelopeValidationError, ValidatedBuiltinObjectSectionInventoryV1,
    read_builtin_object_sections_v1, validate_darwin_arm64_object_envelope_v1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedScoopLirObjectEnvelopeV1 {
    sections: ValidatedBuiltinObjectSectionInventoryV1,
}

impl ValidatedScoopLirObjectEnvelopeV1 {
    pub(in crate::link_object) fn from_sections(
        sections: ValidatedBuiltinObjectSectionInventoryV1,
    ) -> Self {
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
        return crate::link_object::elf::builtin::sections(bytes, target.id())
            .map(|sections| ValidatedScoopLirObjectEnvelopeV1 { sections })
            .map_err(ScoopLirObjectEnvelopeValidationError::Elf);
    }
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Coff64 {
        return crate::link_object::coff::builtin::sections(bytes, target.id())
            .map(|sections| ValidatedScoopLirObjectEnvelopeV1 { sections })
            .map_err(ScoopLirObjectEnvelopeValidationError::Coff);
    }
    let envelope = validate_darwin_arm64_object_envelope_v1(bytes)
        .map_err(ScoopLirObjectEnvelopeValidationError::Envelope)?;
    let sections = read_builtin_object_sections_v1(envelope);
    Ok(ValidatedScoopLirObjectEnvelopeV1 { sections })
}

pub fn read_generated_c_object_sections_v1(
    bytes: &[u8],
    target: scoop_lir::TargetProfileId,
) -> Result<ValidatedBuiltinObjectSectionInventoryV1, GeneratedCBridgeObjectEnvelopeValidationError>
{
    match target {
        scoop_target_data::DARWIN_AARCH64_ID => {
            let envelope = validate_darwin_arm64_object_envelope_v1(bytes)
                .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Envelope)?;
            Ok(read_builtin_object_sections_v1(envelope))
        }
        scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
            crate::link_object::elf::builtin::sections(bytes, target)
                .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Elf)
        }
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
        | scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
        | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID => {
            crate::link_object::coff::builtin::sections(bytes, target)
                .map_err(GeneratedCBridgeObjectEnvelopeValidationError::Coff)
        }
        _ => Err(GeneratedCBridgeObjectEnvelopeValidationError::UnsupportedTarget),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopLirObjectEnvelopeValidationError {
    UnsupportedTarget,
    Elf(crate::link_object::ElfObjectError),
    Coff(crate::link_object::CoffObjectError),
    Envelope(ObjectEnvelopeValidationError),
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
    Coff(crate::link_object::CoffObjectError),
    Envelope(ObjectEnvelopeValidationError),
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
