use super::*;

#[test]
fn actual_elf_program_rejects_changed_alias_startup_permissions_and_stackmaps() {
    let fixture = crate::test_support::fixture();
    let inputs =
        ProgramInputs::new(&fixture.closure, &fixture.runtime, &fixture.profile, &[]).unwrap();
    let ValidatedFinalLinkProfile::Linux(profile) = &fixture.profile else {
        panic!("expected ELF profile")
    };
    let original = std::fs::read(&fixture.output.path).unwrap();
    verify(&original, &inputs, profile).unwrap();
    let file: ElfFile64<'_> = ElfFile64::parse(original.as_slice()).unwrap();
    let file_offset = |address: u64| {
        file.sections()
            .find_map(|section| {
                let relative = address.checked_sub(section.address())?;
                (relative < section.size())
                    .then_some(section.file_range()?.0 as usize + relative as usize)
            })
            .unwrap()
    };
    let symbol = |name| {
        file.symbols()
            .find(|symbol| symbol.name().unwrap() == name)
            .unwrap()
    };
    let reject = |bytes: Vec<u8>, expected: &str| {
        let message = verify(&bytes, &inputs, profile)
            .expect_err("accepted corrupted ELF")
            .to_string();
        assert!(message.contains(expected), "{message}; expected {expected}");
        assert_eq!(std::fs::read(&fixture.output.path).unwrap(), original);
    };

    let import = file
        .dynamic_symbols()
        .find(|symbol| {
            symbol.is_undefined() && !symbol.is_weak() && !symbol.name().unwrap().is_empty()
        })
        .unwrap();
    let name = import.name_bytes().unwrap();
    let at = name.as_ptr() as usize - original.as_ptr() as usize;
    let mut bytes = original.clone();
    bytes[at] = b'!';
    reject(bytes, "unresolved final ELF import");

    let alias = symbol("scoop_td_String");
    let at = alias.elf_symbol() as *const _ as usize - original.as_ptr() as usize + 8;
    let mut bytes = original.clone();
    bytes[at..at + 8].copy_from_slice(&(alias.address() + 8).to_le_bytes());
    reject(bytes, "String alias");

    let at = file_offset(symbol("scoop_program_images").address());
    let mut bytes = original.clone();
    bytes[at] ^= 8;
    reject(bytes, "unexpected binding");

    let relro = file
        .elf_program_headers()
        .iter()
        .find(|p| p.p_type(file.endian()) == elf::PT_GNU_RELRO)
        .unwrap();
    let at = relro as *const _ as usize - original.as_ptr() as usize + 40;
    let mut bytes = original.clone();
    bytes[at..at + 8].copy_from_slice(&0u64.to_le_bytes());
    reject(bytes, "remains writable");

    let at = file_offset(symbol("__scoop_stackmaps_start").address());
    let mut bytes = original.clone();
    bytes[at + 1] = 7;
    reject(bytes, "stackmap payload changed");

    let at = file
        .elf_program_headers()
        .iter()
        .find(|p| p.p_type(file.endian()) == elf::PT_INTERP)
        .unwrap()
        .p_offset(file.endian()) as usize;
    let mut bytes = original.clone();
    bytes[at + 1] = b'!';
    // The target loader basename is the contract, so corrupt that part too.
    let end = bytes[at..].iter().position(|b| *b == 0).unwrap() + at;
    bytes[end - 1] = b'!';
    reject(bytes, "wrong libc interpreter");
}
