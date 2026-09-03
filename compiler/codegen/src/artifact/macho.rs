//! Mach-O section extraction and relocation validation.

use std::collections::BTreeMap;
use std::path::Path;
use std::slice;

use inkwell::llvm_sys::object::LLVMGetSectionName;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::object_file::{LLVMBinaryType, Section};

use crate::CodegenError;
use crate::statepoint::ExpectedSafepoints;

use super::{FunctionRelocation, TextSection, parse_stackmaps};

const STACKMAP_SECTION: &str = "__llvm_stackmaps";

fn macho_section_name(section: &Section<'_>) -> Result<String, CodegenError> {
    // Mach-O section names are fixed 16-byte fields. LLVM's C object API does
    // not provide a length and a full-width name such as `__llvm_stackmaps`
    // has no in-field NUL, so treating the result as a CStr reads into the
    // following segment field. The selected profile proves this fixed-width
    // representation.
    let (raw, _) = unsafe { section.as_mut_ptr() };
    let pointer = unsafe { LLVMGetSectionName(raw) };
    if pointer.is_null() {
        return Err(CodegenError("Mach-O section has no name".to_string()));
    }
    let bytes = unsafe { slice::from_raw_parts(pointer.cast::<u8>(), 16) };
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|error| CodegenError(format!("Mach-O section name is not UTF-8: {error}")))
}

pub(crate) fn verify_macho_stackmaps(
    path: &Path,
    expected: &ExpectedSafepoints,
    expected_version: u8,
) -> Result<(), CodegenError> {
    let memory = MemoryBuffer::create_from_file(path).map_err(|error| {
        CodegenError(format!(
            "read emitted object {} for stackmap verification: {error}",
            path.display()
        ))
    })?;
    let binary = memory.create_binary_file(None).map_err(|error| {
        CodegenError(format!(
            "parse emitted object {} for stackmap verification: {error}",
            path.display()
        ))
    })?;
    if !matches!(
        binary.get_binary_type(),
        LLVMBinaryType::LLVMBinaryTypeMachO64L
    ) {
        return Err(CodegenError(
            "Darwin/AArch64 backend emitted a non-Mach-O64LE object".to_string(),
        ));
    }
    let mut sections = binary
        .get_sections()
        .ok_or_else(|| CodegenError("emitted Mach-O has no section table".to_string()))?;
    let mut stackmap = None;
    let mut text = None;
    let mut section_names = Vec::new();
    while let Some(section) = sections.next_section() {
        let name = macho_section_name(&section)?;
        section_names.push(name.clone());
        if name == "__text" {
            if text.is_some() {
                return Err(CodegenError(
                    "emitted Mach-O contains more than one __text section".to_string(),
                ));
            }
            text = Some(TextSection {
                address: section.get_address(),
                bytes: section.get_contents().to_vec(),
            });
            continue;
        }
        if name != STACKMAP_SECTION {
            continue;
        }
        if stackmap.is_some() {
            return Err(CodegenError(
                "emitted Mach-O contains more than one __llvm_stackmaps section".to_string(),
            ));
        }
        let contents = section.get_contents().to_vec();
        let mut function_relocations = BTreeMap::new();
        let mut relocations = section.get_relocations();
        while let Some(relocation) = relocations.next_relocation() {
            let (kind, name) = relocation.get_type();
            let name = name.to_string();
            if kind != 0 || name != "ARM64_RELOC_UNSIGNED" {
                return Err(CodegenError(format!(
                    "__llvm_stackmaps carries unsupported relocation {name} ({kind}) at offset {}",
                    relocation.get_offset()
                )));
            }
            let symbol = relocation.get_symbol();
            let function = FunctionRelocation {
                symbol: symbol
                    .get_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<unnamed>".to_string()),
                address: symbol.get_address(),
            };
            if function_relocations
                .insert(relocation.get_offset(), function)
                .is_some()
            {
                return Err(CodegenError(format!(
                    "__llvm_stackmaps repeats relocation offset {}",
                    relocation.get_offset()
                )));
            }
        }
        stackmap = Some((contents, function_relocations));
    }
    let Some((contents, relocations)) = stackmap else {
        if expected.site_count() == 0 {
            return Ok(());
        }
        return Err(CodegenError(format!(
            "emitted Mach-O has no __llvm_stackmaps section; sections: {section_names:?}"
        )));
    };
    if expected.site_count() == 0 {
        return Err(CodegenError(
            "emitted Mach-O has stackmap records but complete LIR has no safepoints".to_string(),
        ));
    }
    let text =
        text.ok_or_else(|| CodegenError("emitted Mach-O has no __text section".to_string()))?;
    parse_stackmaps(&contents, &relocations, &text, expected, expected_version).map_err(|error| {
        CodegenError(format!(
            "LLVM 22.1 Darwin/AArch64 stackmap invariant failed: {error}"
        ))
    })
}
