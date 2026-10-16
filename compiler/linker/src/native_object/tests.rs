use super::*;

#[test]
fn clang_const_metadata_is_read_only_data() {
    let profile =
        scoop_toolchain::ValidatedFinalLinkProfile::resolve("aarch64-apple-darwin").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("metadata.c");
    let object = directory.path().join("metadata.o");
    std::fs::write(&source, "__attribute__((used, section(\"__DATA_CONST,__const\"))) const unsigned long metadata = 11;\n").unwrap();
    crate::test_support::compile_native(&profile, &source, &object);
    let bytes = std::fs::read(object).unwrap();
    let info = NativeObjectInfo::read_with_toolchain(&bytes, profile.startup_toolchain().profile())
        .unwrap();
    let definition = info.definitions["_metadata"];
    assert_eq!(definition.kind, NativeSymbolKind::Data);
    assert!(definition.read_only && definition.binding == NativeSymbolBinding::Strong);
}
