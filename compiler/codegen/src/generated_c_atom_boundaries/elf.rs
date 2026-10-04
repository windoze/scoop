use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use scoop_lir::LinkageClass;

use super::*;
use crate::elf_object::{ElfObject, error};

pub(super) fn materialize(
    bytes: &[u8],
    atoms: &[(BridgeAtomKind, &DefinitionSymbolPlanV1)],
) -> Result<Vec<u8>, CodegenError> {
    let mut output = ElfObject::read(bytes)?;
    for (kind, definition) in atoms {
        let name = definition.primary_symbol().symbol();
        let primary = output
            .file
            .symbol_by_name(name.as_str())
            .ok_or_else(|| CodegenError(format!("missing ELF bridge definition `{name}`")))?;
        let section_index = primary
            .section_index()
            .ok_or_else(|| CodegenError(format!("ELF bridge `{name}` is undefined")))?;
        let section = output.file.section_by_index(section_index).map_err(error)?;
        let (section_name, symbol_kind) = match kind {
            BridgeAtomKind::Entry => (".text", SymbolKind::Text),
            BridgeAtomKind::Signature => (
                crate::c_bridge::ELF_BRIDGE_SIGNATURE_SECTION,
                SymbolKind::Data,
            ),
            BridgeAtomKind::Context => (".rodata.scoop_ctx", SymbolKind::Data),
        };
        let start = primary.address();
        let end = start
            .checked_add(primary.size())
            .ok_or_else(|| CodegenError("ELF bridge extent overflows".into()))?;
        if primary.is_local()
            || primary.is_weak()
            || primary.kind() != symbol_kind
            || primary.size() == 0
            || end > section.size()
            || section.name().map_err(error)? != section_name
        {
            return Err(CodegenError(format!(
                "ELF bridge `{name}` has an invalid {section_name} extent"
            )));
        }
        if !matches!(kind, BridgeAtomKind::Entry) && (start != 0 || end != 1 || section.size() != 1)
        {
            return Err(CodegenError(format!(
                "ELF bridge descriptor `{name}` is not an isolated one-byte section"
            )));
        }
        output.existing_boundary(name.as_str(), LinkageClass::ConeStrong, false)?;
        let boundary = definition.atom_boundaries()[0];
        for (symbol, value) in [(boundary.start(), start), (boundary.end(), end)] {
            output.boundary(
                symbol.symbol().as_str(),
                section_index,
                value,
                LinkageClass::ConeStrong,
            )?;
        }
    }
    output.finish()
}
