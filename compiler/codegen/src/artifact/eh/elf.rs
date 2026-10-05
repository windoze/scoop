//! Connect ELF FDE/LSDA extents to the common invoke verifier.

use object::{Object, ObjectSection};

use crate::ValidatedBackendProfile;
use crate::artifact::elf::{ElfCode, error};

use super::*;

pub(in crate::artifact) fn verify_elf_eh(
    file: &object::File<'_>,
    code: &ElfCode,
    expected: &ExpectedEh,
    observed_safepoints: &ObservedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    let mut fdes = Vec::new();
    let mut lsda_sections = BTreeMap::new();
    for section in file.sections() {
        let name = section.name().map_err(error)?;
        if name == ".eh_frame" {
            fdes.extend(elf_frame::parse(
                file,
                section.index(),
                profile.architecture(),
            )?);
        } else if name == ".gcc_except_table" || name.starts_with(".gcc_except_table.") {
            if section.relocations().next().is_some() {
                return Err(CodegenError(
                    "ELF catch-all/cleanup LSDA must not contain relocations".into(),
                ));
            }
            lsda_sections.insert(section.index().0, section.data().map_err(error)?);
        }
    }
    let mut by_lsda_section: BTreeMap<usize, Vec<Fde>> = BTreeMap::new();
    let mut seen_functions = BTreeSet::new();
    let mut seen_scoop = BTreeSet::new();
    for fde in fdes {
        let function = code.function_at(fde.function.section, fde.function.offset)?;
        if fde.size != function.text.bytes.len() as u64
            || !seen_functions.insert(function.symbol.clone())
        {
            return Err(CodegenError(format!(
                "ELF FDE for `{}` has an incorrect extent or duplicate entry",
                function.symbol
            )));
        }
        if let Some(lsda) = fde.lsda {
            if expected.function(&function.symbol).is_none() {
                return Err(CodegenError(format!(
                    "ELF has a Scoop FDE for unexpected function `{}`",
                    function.symbol
                )));
            }
            seen_scoop.insert(function.symbol.clone());
            by_lsda_section
                .entry(lsda.section.0)
                .or_default()
                .push(Fde {
                    function_symbol: function.symbol.clone(),
                    function_start: function.text.address,
                    function_size: fde.size,
                    lsda_address: lsda.offset,
                });
        }
    }
    if seen_scoop.len() != expected.function_count() {
        return Err(CodegenError(format!(
            "ELF has {} Scoop FDEs, complete LIR requires {}",
            seen_scoop.len(),
            expected.function_count()
        )));
    }
    for (section, mut fdes) in by_lsda_section {
        let bytes = lsda_sections
            .remove(&section)
            .ok_or_else(|| CodegenError("ELF FDE LSDA targets a non-LSDA section".into()))?;
        fdes.sort_by_key(|fde| fde.lsda_address);
        if fdes.first().is_none_or(|fde| fde.lsda_address != 0) {
            return Err(CodegenError(
                "first ELF LSDA does not start at the beginning of its section".into(),
            ));
        }
        for (index, fde) in fdes.iter().enumerate() {
            let end = fdes
                .get(index + 1)
                .map_or(bytes.len() as u64, |next| next.lsda_address);
            if fde.lsda_address >= end || end > bytes.len() as u64 {
                return Err(CodegenError(
                    "ELF LSDA extent is duplicate or out of range".into(),
                ));
            }
            let function = &code.functions[&fde.function_symbol];
            let expectation = expected
                .function(&fde.function_symbol)
                .expect("Scoop FDE was matched to LIR");
            verify_lsda(
                &bytes[fde.lsda_address as usize..end as usize],
                fde,
                &function.text,
                expectation,
                observed_safepoints,
                profile.eh_profile().encodings(),
                profile.architecture(),
            )?;
        }
    }
    if !lsda_sections.is_empty() {
        return Err(CodegenError(
            "ELF has an LSDA section without a Scoop FDE".into(),
        ));
    }
    Ok(())
}
