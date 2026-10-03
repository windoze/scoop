use super::*;
use crate::{
    link::map::{LinkMap, MapSymbol},
    native_object::{NativeReferenceSection, NativeReferences},
};
use scoop_slib::DarwinArm64RelocationTargetV1 as NativeTarget;

pub(super) fn check(
    image: &FinalImage<'_>,
    inputs: &ProgramInputs<'_>,
    map: &LinkMap,
) -> Result<(), LinkError> {
    for (id, references) in &inputs.native.references {
        if references
            .sections
            .values()
            .all(|section| section.uses.is_empty())
        {
            continue;
        }
        let empty = BTreeMap::new();
        let symbols = map.native.get(id).unwrap_or(&empty);
        let check = Object {
            image,
            references,
            symbols,
            synthesized: &map.synthesized,
        };
        for section in references.sections.values() {
            let mut pages = BTreeMap::<u64, BTreeMap<u32, u64>>::new();
            for use_ in &section.uses {
                let source = section
                    .address
                    .checked_add(u64::from(use_.offset()))
                    .ok_or_else(|| error("native reference address overflow"))?;
                let (anchor, place) =
                    check.locate(section, source, u64::from(use_.shape().width_bytes()))?;
                let shape = ResolvedShape::native(use_.shape(), |target| check.target(target))?;
                instructions::check(
                    image,
                    inputs,
                    &shape,
                    use_.encoded_value(),
                    place,
                    pages.entry(anchor).or_default(),
                )
                .map_err(|err| {
                    error(format!(
                        "native object {id} section {} + {:#x}: {err}",
                        section.name,
                        use_.offset()
                    ))
                })?;
            }
        }
    }
    Ok(())
}

struct Object<'a, 'data> {
    image: &'a FinalImage<'data>,
    references: &'a NativeReferences,
    symbols: &'a BTreeMap<String, Vec<MapSymbol>>,
    synthesized: &'a BTreeMap<String, Vec<MapSymbol>>,
}

impl Object<'_, '_> {
    fn locate(
        &self,
        section: &NativeReferenceSection,
        address: u64,
        width: u64,
    ) -> Result<(u64, u64), LinkError> {
        // Darwin ld materializes TLV descriptors under its synthesized owner.
        // The original TLS symbol still identifies this native object's storage.
        let symbols = if section.tlv_descriptors {
            self.synthesized
        } else {
            self.symbols
        };
        for (base, name) in section
            .anchors
            .iter()
            .rev()
            .filter(|(base, _)| *base <= address)
        {
            let Some(rows) = symbols.get(name) else {
                continue;
            };
            let [row] = rows.as_slice() else {
                return Err(error(format!(
                    "native link map symbol {name} has multiple ranges"
                )));
            };
            let offset = address - base;
            if offset.checked_add(width).is_none_or(|end| end > row.size) {
                continue;
            }
            if !self
                .image
                .symbols
                .get(name)
                .is_some_and(|values| values.contains(&row.address))
            {
                return Err(error(format!(
                    "native link map symbol {name} differs from final symbol table"
                )));
            }
            return Ok((
                *base,
                row.address
                    .checked_add(offset)
                    .ok_or_else(|| error("native final address overflow"))?,
            ));
        }
        Err(error(format!(
            "native reference at {address:#x} has no retained object symbol in link map"
        )))
    }

    fn target(&self, target: NativeTarget) -> Result<Expected, LinkError> {
        let (section, address) = match target {
            NativeTarget::SymbolTableIndex(index) => {
                let symbol = &self.references.symbols[&index];
                if symbol.global {
                    return Ok(Expected::Symbol(symbol.name.clone()));
                }
                (
                    symbol
                        .section
                        .ok_or_else(|| error("native local relocation target has no section"))?,
                    symbol.address,
                )
            }
            NativeTarget::SectionOrdinal(ordinal) => {
                (ordinal, self.references.sections[&ordinal].address)
            }
        };
        let (_, address) = self.locate(&self.references.sections[&section], address, 0)?;
        Ok(Expected::Address(address))
    }
}
