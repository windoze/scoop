use super::*;
use object::ObjectSymbol;

#[test]
fn linux_c_drivers_compile_pic_and_tls_with_the_selected_libc() {
    for target in [
        LirTargetProfile::LINUX_X86_64_GNU,
        LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        let invocation = resolve_linux_c_toolchain(target, None, None).unwrap();
        assert_eq!(invocation.profile().contract().target(), &target.wire_id());
        assert!(invocation.profile().contract().deployment().is_err());
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("test.c");
        let object = directory.path().join("test.o");
        std::fs::write(
            &source,
            "_Thread_local long test_tls;\nlong *address(void) { return &test_tls; }\n",
        )
        .unwrap();
        run(&mut invocation.object_compilation_command(&source, &object)).unwrap();
        let bytes = std::fs::read(object).unwrap();
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let tls = file.symbol_by_name("test_tls").unwrap();
        assert_eq!(tls.kind(), object::SymbolKind::Tls);
        assert!(tls.section_index().is_some());
        assert_eq!(tls.size(), 8);
        assert!(file.symbol_by_name("address").is_some());
        assert!(file.symbol_by_name("_address").is_none());
    }
}

#[test]
fn explicit_c_drivers_cannot_silently_select_the_other_libc() {
    let musl = resolve_linux_c_toolchain(
        LirTargetProfile::LINUX_X86_64_MUSL,
        Some(Path::new("gcc")),
        None,
    )
    .unwrap_err();
    assert!(
        musl.to_string()
            .contains("musl target cannot use glibc headers"),
        "{musl}"
    );
    let gnu = resolve_linux_c_toolchain(
        LirTargetProfile::LINUX_X86_64_GNU,
        Some(Path::new("musl-gcc")),
        None,
    )
    .unwrap_err();
    assert!(
        gnu.to_string()
            .contains("glibc target requires glibc headers"),
        "{gnu}"
    );
}

#[test]
fn missing_compiler_and_native_sysroot_are_diagnosed_before_compilation() {
    let target = LirTargetProfile::LINUX_X86_64_MUSL;
    let directory = tempfile::tempdir().unwrap();
    assert!(
        resolve_linux_c_toolchain(target, Some(&directory.path().join("missing-cc")), None)
            .is_err()
    );
    assert!(
        resolve_linux_c_toolchain(
            target,
            None,
            Some(&directory.path().join("missing-sysroot"))
        )
        .is_err()
    );
}
