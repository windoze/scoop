use super::*;

#[test]
fn clang_const_metadata_is_read_only_data_and_cannot_be_instruction_storage() {
    let profile =
        scoop_toolchain::ValidatedFinalLinkProfile::resolve("aarch64-apple-darwin").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("metadata.c");
    let object = directory.path().join("metadata.o");
    std::fs::write(&source, "__attribute__((used, section(\"__DATA_CONST,__const\"))) const unsigned long metadata = 11;\n").unwrap();
    crate::test_support::compile_native(&profile, &source, &object);
    let mut bytes = std::fs::read(object).unwrap();
    let info = NativeObjectInfo::read_with_toolchain(&bytes, profile.startup_toolchain().profile())
        .unwrap();
    let definition = info.definitions["_metadata"];
    assert_eq!(definition.kind, NativeSymbolKind::Data);
    assert!(definition.read_only && !definition.weak);
    let file: MachOFile64<'_> = MachOFile64::parse(bytes.as_slice()).unwrap();
    let section = file.section_by_name("__const").unwrap();
    let flags = &section.macho_section().flags;
    let offset = flags as *const _ as usize - bytes.as_ptr() as usize;
    let invalid = flags.get(file.endian()) | macho::S_ATTR_PURE_INSTRUCTIONS;
    bytes[offset..offset + 4].copy_from_slice(&invalid.to_le_bytes());
    assert!(
        NativeObjectInfo::read_with_toolchain(&bytes, profile.startup_toolchain().profile())
            .is_err()
    );
}
