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
        let boundary = definition.atom_boundaries()[0];
        for (symbol, value) in [(boundary.start(), start), (boundary.end(), end)] {
            output.boundary(symbol.symbol().as_str(), section, value)?;
        }
    }
    output.finish()
}
