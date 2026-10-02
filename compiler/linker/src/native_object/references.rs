use super::*;
use scoop_slib::{
    DarwinArm64RelocationShapeV1 as Shape, DarwinArm64RelocationTargetV1 as Target,
    ObservedMachORelocationV1, decode_darwin_arm64_section_relocations_v1,
};
use std::num::NonZeroU32;

pub(crate) struct NativeReferenceSymbol {
    pub name: String,
    pub global: bool,
    pub section: Option<NonZeroU32>,
    pub address: u64,
}

pub(crate) struct NativeReferenceSection {
    pub name: String,
    pub tlv_descriptors: bool,
    pub address: u64,
    pub anchors: Vec<(u64, String)>,
    pub uses: Vec<ObservedMachORelocationV1>,
}

pub(crate) struct NativeReferences {
    pub symbols: BTreeMap<u32, NativeReferenceSymbol>,
    pub sections: BTreeMap<NonZeroU32, NativeReferenceSection>,
}

impl NativeReferences {
    pub(super) fn read(file: &MachOFile64<'_>) -> Result<Self, LinkError> {
        let section_count = u32::try_from(file.sections().count()).map_err(error)?;
        let symbol_count = u32::try_from(file.macho_symbol_table().len()).map_err(error)?;
        let mut symbols = BTreeMap::new();
        for symbol in file.symbols() {
            let section = symbol
                .section_index()
                .map(|index| {
                    u32::try_from(index.0)
                        .ok()
                        .and_then(NonZeroU32::new)
                        .ok_or_else(|| error("invalid native symbol section"))
                })
                .transpose()?;
            if let Some(index) = section {
                let containing = file
                    .section_by_index(object::SectionIndex(index.get() as usize))
                    .map_err(error)?;
                if symbol
                    .address()
                    .checked_sub(containing.address())
                    .is_none_or(|offset| offset > containing.size())
                {
                    return Err(error("native symbol is outside its section"));
                }
            }
            symbols.insert(
                u32::try_from(symbol.index().0).map_err(error)?,
                NativeReferenceSymbol {
                    name: symbol.name().map_err(error)?.to_owned(),
                    global: symbol.is_global(),
                    section,
                    address: symbol.address(),
                },
            );
        }
        let mut sections = BTreeMap::new();
        for section in file.sections() {
            let ordinal = NonZeroU32::new(u32::try_from(section.index().0).map_err(error)?)
                .ok_or_else(|| error("invalid native section ordinal"))?;
            let name = section.name().map_err(error)?.to_owned();
            let data = section.data().map_err(error)?;
            let raw = section.macho_relocations().map_err(error)?;
            let relocations = decode_darwin_arm64_section_relocations_v1(
                ordinal,
                data,
                object::bytes_of_slice(raw),
                section_count,
                symbol_count,
            )
            .map_err(|err| error(format!("native section {name}: {err}")))?;
            if name == "__compact_unwind" && data.len() % 32 != 0 {
                return Err(error("native compact unwind record is truncated"));
            }
            let tlv_descriptors = section.macho_section().flags.get(file.endian())
                & macho::SECTION_TYPE
                == macho::S_THREAD_LOCAL_VARIABLES;
            if tlv_descriptors && data.len() % 24 != 0 {
                return Err(error("native TLV descriptor is truncated"));
            }
            let mut anchors: Vec<_> = symbols
                .values()
                .filter(|symbol| symbol.section == Some(ordinal))
                .map(|symbol| (symbol.address, symbol.name.clone()))
                .collect();
            anchors.sort();
            // Local references do not enter global symbol resolution. Unwind
            // records are rebuilt by ld; debug sections have no runtime uses.
            let uses = if matches!(name.as_str(), "__compact_unwind" | "__eh_frame")
                || section.kind() == object::SectionKind::Debug
            {
                Vec::new()
            } else {
                relocations
                    .into_iter()
                    .filter(|relocation| {
                        let global = |target| match target {
                            Target::SymbolTableIndex(index) => symbols[&index].global,
                            Target::SectionOrdinal(_) => false,
                        };
                        match relocation.shape() {
                            Shape::Subtractor64 {
                                minuend,
                                subtrahend,
                            } => global(minuend) || global(subtrahend),
                            Shape::Unsigned64 { target }
                            | Shape::Branch26 { target }
                            | Shape::Page21 { target, .. }
                            | Shape::PageOffset12 { target, .. }
                            | Shape::GotLoadPage21 { target }
                            | Shape::GotLoadPageOffset12 { target }
                            | Shape::PointerToGot32 { target }
                            | Shape::TlvpLoadPage21 { target }
                            | Shape::TlvpLoadPageOffset12 { target } => global(target),
                        }
                    })
                    .collect()
            };
            sections.insert(
                ordinal,
                NativeReferenceSection {
                    name,
                    tlv_descriptors,
                    address: section.address(),
                    anchors,
                    uses,
                },
            );
        }
        Ok(Self { symbols, sections })
    }
}
