use super::*;

mod dynamic;
mod native;
use crate::test_support as support;

#[test]
fn actual_executable_corruption_is_rejected_before_publication() {
    let fixture = support::fixture();
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
    let map = native::symbol_ranges(&inputs, &original);
    verify(&original, &inputs, &startup, &fixture.profile, &map).unwrap();
    for (artifact, symbols) in fixture.closure.artifacts() {
        let definitions = artifact
            .production()
            .canonical_definitions()
            .plans()
            .iter()
            .map(|plan| plan.definition_plan())
            .collect();
        let candidate = symbols.final_objects().runtime_images().fingerprint();
        assert_eq!(
            candidate.fingerprint_for_definitions(&definitions).unwrap(),
            candidate.fingerprint()
        );
    }
    let reject = |name: &str, bytes: Vec<u8>, expected: &str| {
        let error = match verify(&bytes, &inputs, &startup, &fixture.profile, &map) {
            Err(error) => error.to_string(),
            Ok(()) => panic!("{name}: corrupted executable was accepted"),
        };
        assert!(error.contains(expected), "{name}: {error}");
        assert_eq!(std::fs::read(&fixture.output.path).unwrap(), original);
    };

    let file: MachOFile64<'_> = MachOFile64::parse(original.as_slice()).unwrap();
    let image = file
        .symbols()
        .find(|symbol| symbol.name().ok() == Some(inputs.images[0].as_str()))
        .unwrap();
    let section = file
        .section_by_index(image.section_index().unwrap())
        .unwrap();
    let offset = (section.file_range().unwrap().0 + image.address() - section.address()) as usize;
    for (field, expected) in [
        (96, "differs from its selected contents"),
        (232, "incorrect selected count"),
    ] {
        let mut bytes = original.clone();
        bytes[offset + field] ^= 1;
        reject("selected image", bytes, expected);
    }

    let mut bytes = original.clone();
    let main = command(&bytes, macho::LC_MAIN);
    bytes[main + 8..main + 16].copy_from_slice(&0u64.to_le_bytes());
    reject("entry", bytes, "LC_MAIN");

    let mut bytes = original.clone();
    let library = find(&bytes, b"/usr/lib/libSystem.B.dylib");
    bytes[library + 18] = b'X';
    reject("provider", bytes, "unexpected final dynamic provider");

    let mut bytes = original.clone();
    let library = command(&bytes, macho::LC_LOAD_DYLIB);
    bytes[library + 16..library + 20].copy_from_slice(&0u32.to_le_bytes());
    reject("provider version", bytes, "version differs");

    let mut bytes = original.clone();
    let signature = command(&bytes, macho::LC_CODE_SIGNATURE);
    let offset = word(&bytes, signature + 8) as usize;
    bytes[offset + 8..offset + 12].copy_from_slice(&0u32.to_be_bytes());
    reject("signature", bytes, "CodeDirectory");

    let mut bytes = original.clone();
    let alias = symbol(&bytes, "_scoop_td_String");
    let old = u64::from_le_bytes(bytes[alias + 8..alias + 16].try_into().unwrap());
    bytes[alias + 8..alias + 16].copy_from_slice(&(old + 8).to_le_bytes());
    reject("alias", bytes, "_scoop_td_String");

    let mut bytes = original.clone();
    let local = symbol(&bytes, crate::startup::IMAGE_ARRAY);
    bytes[local + 4] |= macho::N_EXT;
    reject(
        "extra global",
        bytes,
        "unexpected final non-local definition",
    );

    let mut bytes = original.clone();
    let stackmap_name = find(&bytes, b"__llvm_stackmaps");
    bytes[stackmap_name + 2] = b'x';
    reject("stackmap section", bytes, "stackmaps");

    let mut bytes = original.clone();
    let file: MachOFile64<'_> = MachOFile64::parse(bytes.as_slice()).unwrap();
    let section = file.section_by_name("__llvm_stackmaps").unwrap();
    let (offset, _) = section.file_range().unwrap();
    bytes[offset as usize + 1] = 7;
    reject("stackmap payload", bytes, "stackmap payload changed");

    let mut bytes = original.clone();
    let dyld = command(&bytes, macho::LC_DYLD_INFO_ONLY);
    let rebase = word(&bytes, dyld + 8) as usize;
    bytes[rebase] = 0xff;
    reject("rebase", bytes, "rebase opcode");

    let mut bytes = original.clone();
    let dyld = command(&bytes, macho::LC_DYLD_INFO_ONLY);
    let export = word(&bytes, dyld + 40) as usize;
    bytes[export] = 0x7f;
    reject("export trie", bytes, "export");

    let mut bytes = original.clone();
    let file: MachOFile64<'_> = MachOFile64::parse(bytes.as_slice()).unwrap();
    let imports: Vec<_> = file
        .imports()
        .unwrap()
        .into_iter()
        .map(|import| import.name().to_vec())
        .collect();
    let mut renamed = false;
    for import in imports {
        for stream in [16, 24, 32] {
            let start = word(&bytes, dyld + stream) as usize;
            let size = word(&bytes, dyld + stream + 4) as usize;
            if let Some(relative) = bytes[start..start + size]
                .windows(import.len())
                .position(|name| name == import)
            {
                bytes[start + relative] = b'!';
                renamed = true;
                break;
            }
        }
        if renamed {
            break;
        }
    }
    assert!(renamed);
    reject("binding", bytes, "unexpected final binding");

    let mut bytes = original.clone();
    let mut odr = inputs.definitions.iter().filter_map(|(name, owner)| {
        matches!(
            owner,
            DefinitionOwner::Scoop(LinkDefinitionOwnerV1::OdrDefinition(_))
        )
        .then_some(name)
    });
    let first = symbol(&bytes, odr.next().unwrap());
    let second = symbol(&bytes, odr.next().unwrap());
    let address = bytes[first + 8..first + 16].to_vec();
    bytes[second + 8..second + 16].copy_from_slice(&address);
    reject("ODR winner", bytes, "final export");
}

fn command(bytes: &[u8], tag: u32) -> usize {
    let mut offset = 32;
    for _ in 0..word(bytes, 16) {
        if word(bytes, offset) == tag {
            return offset;
        }
        offset += word(bytes, offset + 4) as usize;
    }
    panic!("missing command {tag:#x}")
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn find(bytes: &[u8], pattern: &[u8]) -> usize {
    bytes
        .windows(pattern.len())
        .position(|value| value == pattern)
        .unwrap()
}
fn symbol(bytes: &[u8], name: &str) -> usize {
    let file: MachOFile64<'_> = MachOFile64::parse(bytes).unwrap();
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().unwrap() == name)
        .unwrap();
    symbol.macho_symbol() as *const _ as usize - bytes.as_ptr() as usize
}
