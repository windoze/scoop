//! Own LLVM's indirect EH personality pointer as a callable constant atom.

use object::{Object, ObjectSymbol, SectionIndex, SymbolKind};
use scoop_lir::{
    AtomBoundarySymbolsV1, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
};

use super::*;

pub(super) fn atom(plan: &DefinitionSymbolPlanV1) -> Result<ObjectDefinitionAtomId, CodegenError> {
    let StrongDefinitionEntityKind::CallableBody(body) = plan.owner().kind() else {
        return Err(error("personality contribution has no callable owner"));
    };
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan.definition_plan(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::CallableBody(body),
    ))
    .map_err(error)
}

pub(super) fn materialize(
    output: &mut ElfObject<'_>,
    boundary: AtomBoundarySymbolsV1,
) -> Result<SectionIndex, CodegenError> {
    const NAME: &str = "DW.ref.scoop_eh_personality";
    let symbol = output
        .file
        .symbol_by_name(NAME)
        .ok_or_else(|| error("missing LLVM personality indirection"))?;
    let section = symbol
        .section_index()
        .ok_or_else(|| error("undefined personality indirection"))?;
    if symbol.kind() != SymbolKind::Data
        || symbol.address() != 0
        || symbol.size() != 8
        || !symbol.is_weak()
        || output.file.section_by_index(section).map_err(error)?.size() != 8
    {
        return Err(error(
            "invalid LLVM personality contribution extent or binding",
        ));
    }
    output.detach_personality_group(section)?;
    output.rename_section(section, ".data.rel.ro.scoop.personality")?;
    output.rename_symbol(
        NAME,
        boundary.start().symbol().as_str(),
        boundary.start().linkage(),
    )?;
    output.boundary(
        boundary.end().symbol().as_str(),
        section,
        8,
        boundary.end().linkage(),
    )?;
    Ok(section)
}
