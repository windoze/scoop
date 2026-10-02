use super::*;
use crate::link::map::{LinkMap, MapSymbol};
use scoop_slib::{DarwinArm64RelocationShapeV1 as Shape, DarwinArm64RelocationTargetV1 as Target};

#[test]
fn actual_native_call_pointer_and_map_address_corruption_are_rejected() {
    let fixture = support::native_fixture();
    let inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
    )
    .unwrap();
    let startup =
        StartupObject::build(&inputs, &fixture.profile, fixture.directory.path()).unwrap();
    let original = std::fs::read(&fixture.output.path).unwrap();
    let map = symbol_ranges(&inputs, &original);
    verify(&original, &inputs, &startup, &fixture.profile, &map).unwrap();
    let output = std::process::Command::new(&fixture.output.path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    let reject = |bytes: &[u8], map: &LinkMap, expected: &str| {
        let message = verify(bytes, &inputs, &startup, &fixture.profile, map)
            .err()
            .unwrap()
            .to_string();
        assert!(message.contains(expected), "{message}");
        assert_eq!(std::fs::read(&fixture.output.path).unwrap(), original);
    };

    let file: MachOFile64<'_> = MachOFile64::parse(original.as_slice()).unwrap();
    let address = |name: &str| {
        file.symbols()
            .find(|symbol| symbol.name().unwrap() == name)
            .unwrap()
            .address()
    };
    let offset = |address: u64| {
        file.sections()
            .find_map(|section| {
                let relative = address.checked_sub(section.address())?;
                let (offset, size) = section.file_range()?;
                (relative < size).then_some((offset + relative) as usize)
            })
            .unwrap()
    };
    let mut call = None;
    for references in inputs.native.references.values() {
        for section in references.sections.values() {
            for use_ in &section.uses {
                if let Shape::Branch26 {
                    target: Target::SymbolTableIndex(index),
                } = use_.shape()
                    && references.symbols[&index].name == "_m23_target"
                {
                    let source = section.address + u64::from(use_.offset());
                    let (base, name) = section
                        .anchors
                        .iter()
                        .rev()
                        .find(|(base, name)| {
                            *base <= source
                                && file.symbols().any(|symbol| symbol.name().unwrap() == name)
                        })
                        .unwrap();
                    call = Some(offset(address(name) + source - base));
                }
            }
        }
    }
    let mut bytes = original.clone();
    let call = call.unwrap();
    let instruction = word(&bytes, call);
    bytes[call..call + 4].copy_from_slice(&(instruction + 1).to_le_bytes());
    reject(&bytes, &map, "branch at");

    let mut bytes = original.clone();
    let pointer = offset(address("_m23_pointer"));
    bytes[pointer..pointer + 8].copy_from_slice(&address("_m23_final").to_le_bytes());
    reject(&bytes, &map, "pointer at");

    let mut wrong = symbol_ranges(&inputs, &original);
    for symbols in wrong.native.values_mut() {
        for row in symbols.get_mut("_local_call").unwrap() {
            row.address += 4;
        }
    }
    reject(&original, &wrong, "differs from final symbol table");
    reject(&original, &LinkMap::default(), "no retained object symbol");
}

// The production link has already checked the actual ld map. For byte-mutation
// tests retain its ordinary native symbol ranges using the unchanged input and
// output tables, without invoking ld again or adding a production test hook.
fn symbol_ranges(inputs: &ProgramInputs<'_>, bytes: &[u8]) -> LinkMap {
    let file: MachOFile64<'_> = MachOFile64::parse(bytes).unwrap();
    let final_symbols: BTreeMap<_, _> = file
        .symbols()
        .filter(|symbol| symbol.is_definition())
        .map(|symbol| (symbol.name().unwrap().to_owned(), symbol.address()))
        .collect();
    let mut map = LinkMap::default();
    for input in &inputs.objects {
        let crate::program::ObjectOrigin::Native(id) = input.origin else {
            continue;
        };
        let source: MachOFile64<'_> = MachOFile64::parse(input.bytes()).unwrap();
        for section in source.sections() {
            let mut symbols: Vec<_> = source
                .symbols()
                .filter(|symbol| symbol.section_index() == Some(section.index()))
                .filter(|symbol| final_symbols.contains_key(symbol.name().unwrap()))
                .collect();
            symbols.sort_by_key(|symbol| symbol.address());
            for (index, symbol) in symbols.iter().enumerate() {
                let end = symbols[index + 1..]
                    .iter()
                    .find(|next| next.address() > symbol.address())
                    .map_or(section.address() + section.size(), ObjectSymbol::address);
                let name = symbol.name().unwrap();
                map.native.entry(id).or_default().insert(
                    name.to_owned(),
                    vec![MapSymbol {
                        address: final_symbols[name],
                        size: end - symbol.address(),
                    }],
                );
            }
        }
    }
    map
}
