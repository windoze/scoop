use object::ObjectSymbol;
use scoop_lir::LinkageClass;

use super::*;
use crate::elf_object::ElfObject;

pub(super) fn materialize(
    bytes: &[u8],
    atoms: &[&DefinitionSymbolPlanV1],
) -> Result<Vec<u8>, CodegenError> {
    let mut output = ElfObject::read(bytes)?;
    for definition in atoms {
        let name = definition.primary_symbol().symbol();
        let primary = output
            .original_symbol(name.as_str())
            .ok_or_else(|| CodegenError(format!("missing ELF bridge definition `{name}`")))?;
        let section_index = primary
            .section_index()
            .ok_or_else(|| CodegenError(format!("ELF bridge `{name}` is undefined")))?;
        let start = primary.address();
        let end = start
            .checked_add(primary.size())
            .ok_or_else(|| CodegenError("ELF bridge extent overflows".into()))?;
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
