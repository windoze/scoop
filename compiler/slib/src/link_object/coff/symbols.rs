use object::read::coff::ImageSymbol;
use object::{ObjectSymbol, SymbolFlags, SymbolKind, pe};

use super::*;

pub(super) fn read<'data, H: CoffHeader>(
    file: &CoffFile<'data, &'data [u8], H>,
    sections: &[ObservedObjectSectionV1],
) -> Result<Vec<ObservedObjectSymbolV1>, CoffObjectError> {
    let table = file.coff_symbol_table();
    let mut symbols = Vec::with_capacity(table.len());
    let mut selections = vec![0; sections.len()];
    for symbol in file.symbols() {
        let raw = symbol.coff_symbol();
        let end = symbol
            .index()
            .0
            .checked_add(1 + usize::from(raw.number_of_aux_symbols()))
            .filter(|end| *end <= table.len())
            .ok_or_else(|| error("auxiliary symbol outside COFF symbol table"))?;
        let section_ordinal = symbol.section_index().map(ordinal).transpose()?;
        if let Some(ordinal) = section_ordinal {
            let section = sections
                .get(ordinal.get() as usize - 1)
                .ok_or_else(|| error("symbol references a missing COFF section"))?;
            if u64::from(raw.value()) > section.byte_size() {
                return Err(error("symbol value is outside its COFF section"));
            }
        }
        let auxiliary = if raw.has_aux_weak_external() {
            let weak = table.aux_weak_external(symbol.index()).map_err(error)?;
            CoffSymbolAuxiliaryV1::WeakExternal {
                default_symbol: weak.weak_default_sym_index.get(object::LittleEndian),
                search: weak.weak_search_type.get(object::LittleEndian),
            }
        } else if raw.has_aux_section() {
            table.aux_section(symbol.index()).map_err(error)?;
            let SymbolFlags::CoffSection {
                selection,
                associative_section,
            } = symbol.flags()
            else {
                return Err(error("missing COFF section auxiliary record"));
            };
            let associative_section = associative_section.map(ordinal).transpose()?;
            if associative_section.is_some_and(|ordinal| ordinal.get() as usize > sections.len()) {
                return Err(error("COMDAT association references a missing section"));
            }
            if let Some(ordinal) = section_ordinal {
                selections[ordinal.get() as usize - 1] = selection;
            }
            CoffSymbolAuxiliaryV1::Section {
                selection,
                associative_section,
            }
        } else {
            CoffSymbolAuxiliaryV1::None
        };
        let kind = if symbol.is_undefined() || symbol.is_weak() {
            ObjectSymbolKindV1::ExternalUndefined
        } else if symbol.kind() == SymbolKind::Section {
            ObjectSymbolKindV1::SectionBase
        } else if symbol.kind() == SymbolKind::File
            || symbol.section_index().is_none() && !symbol.is_common()
        {
            ObjectSymbolKindV1::FileMetadata
        } else if raw.storage_class() == pe::IMAGE_SYM_CLASS_EXTERNAL {
            ObjectSymbolKindV1::ExternalStrongDefinition
        } else {
            ObjectSymbolKindV1::LocalSectionDefinition
        };
        symbols.push(ObservedObjectSymbolV1 {
            table_index: u32::try_from(symbol.index().0).map_err(error)?,
            name: symbol.name_bytes().map_err(error)?.to_vec(),
            kind,
            section_ordinal,
            value: u64::from(raw.value()),
            attributes: ObjectSymbolAttributesV1::Coff {
                storage_class: raw.storage_class(),
                symbol_type: raw.typ(),
                auxiliary,
            },
        });
        while symbols.len() < end {
            symbols.push(ObservedObjectSymbolV1 {
                table_index: u32::try_from(symbols.len()).map_err(error)?,
                name: Vec::new(),
                kind: ObjectSymbolKindV1::FileMetadata,
                section_ordinal: None,
                value: 0,
                attributes: ObjectSymbolAttributesV1::Coff {
                    storage_class: 0,
                    symbol_type: 0,
                    auxiliary: CoffSymbolAuxiliaryV1::AuxiliaryRecord,
                },
            });
        }
    }
    for symbol in &mut symbols {
        if symbol.kind == ObjectSymbolKindV1::ExternalStrongDefinition
            && symbol.section_ordinal.is_some_and(|ordinal| {
                matches!(
                    selections[ordinal.get() as usize - 1],
                    pe::IMAGE_COMDAT_SELECT_ANY
                        | pe::IMAGE_COMDAT_SELECT_SAME_SIZE
                        | pe::IMAGE_COMDAT_SELECT_EXACT_MATCH
                        | pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE
                        | pe::IMAGE_COMDAT_SELECT_LARGEST
                        | pe::IMAGE_COMDAT_SELECT_NEWEST
                )
            })
        {
            symbol.kind = ObjectSymbolKindV1::ExternalWeakDefinition;
        }
    }
    for symbol in &symbols {
        if let ObjectSymbolAttributesV1::Coff {
            auxiliary: CoffSymbolAuxiliaryV1::WeakExternal { default_symbol, .. },
            ..
        } = symbol.attributes
        {
            require_symbol(&symbols, default_symbol)?;
        }
    }
    Ok(symbols)
}

pub(super) fn require_symbol(
    symbols: &[ObservedObjectSymbolV1],
    index: u32,
) -> Result<(), CoffObjectError> {
    let symbol = symbols
        .get(index as usize)
        .ok_or_else(|| error("COFF symbol reference outside table"))?;
    if matches!(
        symbol.attributes,
        ObjectSymbolAttributesV1::Coff {
            auxiliary: CoffSymbolAuxiliaryV1::AuxiliaryRecord,
            ..
        }
    ) {
        return Err(error("COFF reference targets an auxiliary symbol record"));
    }
    Ok(())
}
