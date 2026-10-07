//! Mach-O section extraction and relocation validation.

use std::collections::BTreeMap;
use std::path::Path;

use object::read::macho::MachHeader as _;
use object::{
    Architecture, BinaryFormat, Object, ObjectKind, ObjectSection, ObjectSymbol, RelocationFlags,
    RelocationTarget, SymbolKind, macho,
};

use crate::CodegenError;
use crate::statepoint::ExpectedSafepoints;
use crate::target::LsdaEncodingProfile;

use super::eh::{self, EhRelocation, EhSection, ExpectedEh};
use super::{FunctionRelocation, ObservedSafepoints, TextSection, parse_stackmaps};

const STACKMAP_SECTION: &str = "__llvm_stackmaps";
const EH_FRAME_SECTION: &str = "__eh_frame";
const GCC_EXCEPT_TABLE_SECTION: &str = "__gcc_except_tab";

pub(crate) fn verify_macho_artifact(
    path: &Path,
    expected_safepoints: &ExpectedSafepoints,
    expected_eh: &ExpectedEh,
    expected_stackmap_version: u8,
    eh_encodings: LsdaEncodingProfile,
) -> Result<(), CodegenError> {
    let bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!("read emitted object {}: {error}", path.display()))
    })?;
    let file = object::File::parse(bytes.as_slice()).map_err(|error| {
        CodegenError(format!("parse emitted object {}: {error}", path.display()))
    })?;
    let ProfileSections {
        text,
        eh_frame,
        gcc_except_tab,
    } = extract_profile_sections(&file)?;
    let mut stackmap = None;
    let mut section_names = Vec::new();
    for section in file.sections() {
        let name = section
            .name()
            .map_err(|error| CodegenError(format!("Mach-O section name: {error}")))?
            .to_string();
        section_names.push(name.clone());
        if name == STACKMAP_SECTION {
            if stackmap.is_some() {
                return Err(CodegenError(
                    "emitted Mach-O contains more than one __llvm_stackmaps section".to_string(),
                ));
            }
            stackmap = Some(extract_stackmap(&file, &section)?);
        }
    }

    let observed_safepoints = verify_stackmap(
        stackmap,
        text.as_ref(),
        expected_safepoints,
        expected_stackmap_version,
        &section_names,
    )?;
    eh::verify_sections(
        eh_frame.as_ref(),
        gcc_except_tab.as_ref(),
        text.as_ref(),
        expected_eh,
        &observed_safepoints,
        eh_encodings,
    )
    .map_err(|error| {
        CodegenError(format!(
            "LLVM 22.1 Darwin/AArch64 EH artifact invariant failed: {error}"
        ))
    })
}

fn extract_stackmap(
    file: &object::File<'_>,
    section: &object::Section<'_, '_>,
) -> Result<(Vec<u8>, BTreeMap<u64, FunctionRelocation>), CodegenError> {
    let contents = section
        .data()
        .map_err(|error| CodegenError(format!("Mach-O stackmap contents: {error}")))?
        .to_vec();
    let mut function_relocations = BTreeMap::new();
    for (offset, relocation) in section.relocations() {
        if relocation.flags()
            != (RelocationFlags::MachO {
                r_type: macho::ARM64_RELOC_UNSIGNED,
                r_pcrel: false,
                r_length: 3,
            })
        {
            return Err(CodegenError(format!(
                "__llvm_stackmaps carries unsupported relocation {:?} at offset {offset}",
                relocation.flags()
            )));
        }
        let RelocationTarget::Symbol(index) = relocation.target() else {
            return Err(CodegenError(
                "Mach-O stackmap has a non-symbol relocation".into(),
            ));
        };
        let symbol = file
            .symbol_by_index(index)
            .map_err(|error| CodegenError(format!("Mach-O stackmap function symbol: {error}")))?;
        if symbol.kind() != SymbolKind::Text || !symbol.is_definition() || relocation.addend() != 0
        {
            return Err(CodegenError(
                "Mach-O stackmap relocation does not address a defined function".into(),
            ));
        }
        let function = FunctionRelocation {
            symbol: symbol
                .name()
                .map_err(|error| CodegenError(format!("Mach-O stackmap function name: {error}")))?
                .to_string(),
            address: symbol.address(),
        };
        if function_relocations.insert(offset, function).is_some() {
            return Err(CodegenError(format!(
                "__llvm_stackmaps repeats relocation offset {offset}"
            )));
        }
    }
    Ok((contents, function_relocations))
}

struct ProfileSections {
    text: Option<TextSection>,
    eh_frame: Option<EhSection>,
    gcc_except_tab: Option<EhSection>,
}

fn extract_profile_sections(file: &object::File<'_>) -> Result<ProfileSections, CodegenError> {
    if file.format() != BinaryFormat::MachO
        || file.architecture() != Architecture::Aarch64
        || !file.is_little_endian()
        || !file.is_64()
        || file.kind() != ObjectKind::Relocatable
    {
        return Err(CodegenError(
            "EH qualification requires a little-endian Mach-O64/AArch64 object".to_string(),
        ));
    }
    let object::File::MachO64(macho_file) = &file else {
        return Err(CodegenError(
            "EH qualification requires a thin Mach-O64 object".to_string(),
        ));
    };
    let endian = macho_file.endian();
    let cpu_type = macho_file.macho_header().cputype(endian);
    let mut text = None;
    let mut eh_frame = None;
    let mut gcc_except_tab = None;
    for section in macho_file.sections() {
        let name = section.name().map_err(|error| {
            CodegenError(format!("Mach-O section name is not valid UTF-8: {error}"))
        })?;
        if !matches!(name, "__text" | EH_FRAME_SECTION | GCC_EXCEPT_TABLE_SECTION) {
            continue;
        }
        let segment = section.segment_name().map_err(|error| {
            CodegenError(format!("cannot read Mach-O {name} segment name: {error}"))
        })?;
        if segment != Some("__TEXT") {
            return Err(CodegenError(format!(
                "Mach-O {name} belongs to segment {segment:?}, expected __TEXT"
            )));
        }
        let contents = section.data().map_err(|error| {
            CodegenError(format!("cannot read Mach-O {name} contents: {error}"))
        })?;
        if section.size() != contents.len() as u64 {
            return Err(CodegenError(format!(
                "Mach-O {name} file contents do not cover its complete section size"
            )));
        }
        if name == "__text" {
            if text.is_some() {
                return Err(CodegenError(
                    "emitted Mach-O contains more than one __text section".to_string(),
                ));
            }
            text = Some(TextSection {
                address: section.address(),
                bytes: contents.to_vec(),
                non_unwinding_calls: super::copy_calls::non_unwinding_calls(
                    file,
                    &file
                        .section_by_index(section.index())
                        .map_err(|error| CodegenError(format!("Mach-O text section: {error}")))?,
                )?,
            });
            continue;
        }
        let destination = if name == EH_FRAME_SECTION {
            &mut eh_frame
        } else {
            &mut gcc_except_tab
        };
        if destination.is_some() {
            return Err(CodegenError(format!(
                "emitted Mach-O contains more than one {name} section"
            )));
        }
        let raw_relocations = section.macho_relocations().map_err(|error| {
            CodegenError(format!("cannot read raw {name} relocations: {error}"))
        })?;
        validate_raw_relocations(name, raw_relocations, endian, cpu_type)?;
        let mut relocations = Vec::new();
        for (offset, relocation) in section.relocations() {
            let RelocationFlags::MachO {
                r_type,
                r_pcrel,
                r_length,
            } = relocation.flags()
            else {
                return Err(CodegenError(format!(
                    "{name} relocation at offset {offset} has non-Mach-O flags"
                )));
            };
            let RelocationTarget::Symbol(symbol_index) = relocation.target() else {
                return Err(CodegenError(format!(
                    "{name} relocation at offset {offset} does not target a Mach-O symbol"
                )));
            };
            let symbol = macho_file.symbol_by_index(symbol_index).map_err(|error| {
                CodegenError(format!(
                    "cannot resolve {name} relocation target at offset {offset}: {error}"
                ))
            })?;
            if !relocation.has_implicit_addend() || relocation.addend() != 0 {
                return Err(CodegenError(format!(
                    "{name} relocation at offset {offset} does not use the qualified implicit addend form"
                )));
            }
            let symbol_section = symbol
                .section_index()
                .map(|index| {
                    macho_file
                        .section_by_index(index)
                        .and_then(|section| section.name().map(str::to_string))
                        .map_err(|error| {
                            CodegenError(format!(
                                "cannot resolve section for {name} relocation target at offset {offset}: {error}"
                            ))
                        })
                })
                .transpose()?;
            relocations.push(EhRelocation {
                offset,
                r_type,
                r_pcrel,
                r_length,
                symbol: symbol
                    .name()
                    .map_err(|error| {
                        CodegenError(format!(
                            "{name} relocation target at offset {offset} has invalid name: {error}"
                        ))
                    })?
                    .to_string(),
                symbol_address: symbol.address(),
                symbol_section,
                symbol_is_undefined: symbol.is_undefined(),
                symbol_is_local: symbol.is_local(),
                symbol_is_text: symbol.kind() == SymbolKind::Text,
            });
        }
        if relocations.len() != raw_relocations.len() {
            return Err(CodegenError(format!(
                "{name} exposes {} decoded relocations for {} raw entries",
                relocations.len(),
                raw_relocations.len()
            )));
        }
        *destination = Some(EhSection {
            address: section.address(),
            bytes: contents.to_vec(),
            relocations,
        });
    }
    Ok(ProfileSections {
        text,
        eh_frame,
        gcc_except_tab,
    })
}

fn validate_raw_relocations(
    section_name: &str,
    relocations: &[macho::Relocation<object::Endianness>],
    endian: object::Endianness,
    cpu_type: u32,
) -> Result<(), CodegenError> {
    for (index, relocation) in relocations.iter().copied().enumerate() {
        if relocation.r_scattered(endian, cpu_type) {
            return Err(CodegenError(format!(
                "{section_name} raw relocation {index} uses unsupported scattered form"
            )));
        }
        if relocation.info(endian).r_type == macho::ARM64_RELOC_ADDEND {
            return Err(CodegenError(format!(
                "{section_name} raw relocation {index} uses unsupported ARM64_RELOC_ADDEND"
            )));
        }
    }
    Ok(())
}

fn verify_stackmap(
    stackmap: Option<(Vec<u8>, BTreeMap<u64, FunctionRelocation>)>,
    text: Option<&TextSection>,
    expected: &ExpectedSafepoints,
    expected_version: u8,
    section_names: &[String],
) -> Result<ObservedSafepoints, CodegenError> {
    let Some((contents, relocations)) = stackmap else {
        if expected.site_count() == 0 {
            return Ok(ObservedSafepoints::default());
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
    let text = relocations
        .values()
        .map(|function| (function.symbol.clone(), text.clone()))
        .collect();
    parse_stackmaps(
        &contents,
        &relocations,
        &text,
        expected,
        expected_version,
        crate::target::CodeArchitecture::Aarch64,
    )
    .map_err(|error| {
        CodegenError(format!(
            "LLVM 22.1 Darwin/AArch64 stackmap invariant failed: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use object::Endianness;
    use object::macho::{RelocationInfo, ScatteredRelocationInfo};

    use super::*;

    #[test]
    fn raw_eh_relocations_reject_hidden_forms() {
        let endian = Endianness::Little;
        let addend = RelocationInfo {
            r_address: 0,
            r_symbolnum: 0,
            r_pcrel: false,
            r_length: 2,
            r_extern: false,
            r_type: macho::ARM64_RELOC_ADDEND,
        }
        .relocation(endian);
        assert!(
            validate_raw_relocations("__eh_frame", &[addend], endian, macho::CPU_TYPE_ARM64)
                .is_err()
        );

        let scattered = ScatteredRelocationInfo {
            r_address: 0,
            r_type: macho::ARM64_RELOC_UNSIGNED,
            r_length: 3,
            r_pcrel: false,
            r_value: 0,
        }
        .relocation(endian);
        assert!(
            validate_raw_relocations("__eh_frame", &[scattered], endian, macho::CPU_TYPE_ARM64,)
                .is_err()
        );
    }
}
