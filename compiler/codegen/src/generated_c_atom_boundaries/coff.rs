use object::{Object, ObjectSection, ObjectSymbol};

use super::*;
use crate::coff_object::{CoffObject, error};

pub(super) fn materialize(
    bytes: &[u8],
    atoms: &[&DefinitionSymbolPlanV1],
) -> Result<Vec<u8>, CodegenError> {
    let mut output = CoffObject::read(bytes)?;
    for definition in atoms {
        let name = definition.primary_symbol().symbol();
        let (section, start) = output.extent(name.as_str())?;
        let end = output
            .file
            .symbols()
            .filter(|symbol| {
                symbol.section_index() == Some(section)
                    && symbol.address() > start
                    && symbol.is_global()
            })
            .map(|symbol| symbol.address())
            .min()
            .unwrap_or(output.file.section_by_index(section).map_err(error)?.size());
        let mut associated = Vec::new();
        for boundary in definition.atom_boundaries() {
            let (section, start, end) = match boundary.atom_role() {
                scoop_lir::DefinitionAtomRole::Primary => (section, start, end),
                scoop_lir::DefinitionAtomRole::WindowsUnwindTable
                | scoop_lir::DefinitionAtomRole::WindowsUnwindInfo => {
                    let name = if boundary.atom_role()
                        == scoop_lir::DefinitionAtomRole::WindowsUnwindTable
                    {
                        ".pdata"
                    } else {
                        ".xdata"
                    };
                    let unwind = output.file.section_by_name(name).ok_or_else(|| {
                        error(format!("generated-C bridge has no {name} section"))
                    })?;
                    (unwind.index(), 0, unwind.size())
                }
                role => return Err(error(format!("unexpected generated-C COFF atom {role:?}"))),
            };
            for (symbol, value) in [(boundary.start(), start), (boundary.end(), end)] {
                output.boundary(symbol.symbol().as_str(), section, value)?;
            }
            associated.push(section);
        }
        if associated.len() > 1 {
            output.associate(section, &associated, definition.primary_symbol().linkage())?;
        }
    }
    output.finish()
}
