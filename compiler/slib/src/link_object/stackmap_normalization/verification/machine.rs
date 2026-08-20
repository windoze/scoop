//! Architecture dispatch for the artifact-only stackmap reader.

use super::super::architecture::StackmapArchitecture;
use crate::link_object::{
    ParsedLlvmStackmapFunctionV3, ValidatedBuiltinObjectSectionInventoryV1,
    VerifiedStrongDefinitionSymbolV1, VerifiedStrongObjectDefinitionV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StackmapMachineCodeError {
    Aarch64(super::aarch64::DarwinAarch64StackmapMachineCodeError),
    X86_64(super::x86_64::X86_64StackmapMachineCodeError),
}

impl std::fmt::Display for StackmapMachineCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aarch64(error) => error.fmt(formatter),
            Self::X86_64(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for StackmapMachineCodeError {}

pub(super) fn validate_stackmap_function_machine_code(
    bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    definition: &VerifiedStrongObjectDefinitionV1,
    symbol: &VerifiedStrongDefinitionSymbolV1,
    function: &ParsedLlvmStackmapFunctionV3,
) -> Result<Vec<u64>, StackmapMachineCodeError> {
    match StackmapArchitecture::for_target(sections.envelope().target()) {
        StackmapArchitecture::Aarch64 => super::aarch64::validate_stackmap_function_machine_code(
            bytes, sections, definition, symbol, function,
        )
        .map_err(StackmapMachineCodeError::Aarch64),
        StackmapArchitecture::X86_64 => super::x86_64::validate_stackmap_function_machine_code(
            bytes, sections, definition, symbol, function,
        )
        .map_err(StackmapMachineCodeError::X86_64),
    }
}
