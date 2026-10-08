use super::*;
use scoop_identity::{
    CanonicalNativeLibraryName, NativeLibraryGrouping, NativeLibraryKind, NativeLinkRequirementId,
    NativeLinkRequirementKey,
};
use scoop_process::CommandExt;

#[test]
fn real_system_libraries_reuse_fixed_gnu_providers() {
    check_system_libraries("x86_64-unknown-linux-gnu", false);
}

#[test]
fn real_system_libraries_reuse_fixed_musl_libc() {
    check_system_libraries("x86_64-unknown-linux-musl", true);
}

fn check_system_libraries(target: &str, musl: bool) {
    let profile = ValidatedFinalLinkProfile::resolve(target).unwrap();
    let mut requirements = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for name in ["c", "m"] {
        let key = NativeLinkRequirementKey::for_target(
            profile.target().wire_id(),
            CanonicalNativeLibraryName::new(name).unwrap(),
            NativeLibraryKind::TargetDefault,
            NativeLibraryGrouping::Independent,
        );
        let id = NativeLinkRequirementId::from_key(&key).unwrap();
        requirements.insert(id, (key, vec!["system-library-test".to_owned()]));
        ids.insert(name, id);
    }
    let mut native = NativeInputs::read(requirements, &[], &profile).unwrap();
    let namespace = ElfNamespace::read(&mut native, &[], &profile).unwrap();
    for (name, symbol) in [("c", "puts"), ("m", "cos")] {
        let library = &native.libraries[&ids[name]];
        let candidates = namespace.candidates(symbol, Some(&library.inputs), library.system_alias);
        assert_eq!(candidates.len(), 1, "{name}: {symbol}");
        assert!(matches!(
            candidates[0],
            NativeBinding::Elf(ElfBinding::System(_))
        ));
        assert!(
            namespace
                .candidates(
                    "_Unwind_RaiseException",
                    Some(&library.inputs),
                    library.system_alias
                )
                .is_empty()
        );
    }
    assert_eq!(native.libraries[&ids["m"]].system_alias, musl);
    assert!(
        namespace
            .roots
            .iter()
            .all(|id| !namespace.system_inputs.contains_key(id))
    );
}

#[test]
fn real_elf_flat_namespace_rejects_incompatible_explicit_library_bindings() {
    let profile = ValidatedFinalLinkProfile::resolve("x86_64-unknown-linux-gnu").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-native-link/dynamic");
    let mut libraries = BTreeMap::new();
    let mut ids = Vec::new();
    for name in ["a", "b"] {
        let library = format!("m23_{name}");
        let result = profile
            .startup_toolchain()
            .driver_command()
            .args(["-shared", "-fPIC"])
            .arg(fixtures.join(format!("{name}.c")))
            .arg("-o")
            .arg(path.join(format!("lib{library}.so")))
            .scoop_output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let key = NativeLinkRequirementKey::for_target(
            profile.target().wire_id(),
            CanonicalNativeLibraryName::new(&library).unwrap(),
            NativeLibraryKind::Dynamic,
            NativeLibraryGrouping::Independent,
        );
        let id = NativeLinkRequirementId::from_key(&key).unwrap();
        libraries.insert(id, (key, vec![name.to_owned()]));
        ids.push(id);
    }
    let roots = vec![path.to_owned()];
    let mut native = NativeInputs::read(libraries, &roots, &profile).unwrap();
    let mut namespace = ElfNamespace::read(&mut native, &roots, &profile).unwrap();
    for (symbol, library) in [("m23_foo", ids[0]), ("m23_bar", ids[1])] {
        let mut candidates =
            namespace.candidates(symbol, Some(&native.libraries[&library].inputs), false);
        assert_eq!(candidates.len(), 1);
        let NativeBinding::Elf(binding) = candidates.pop().unwrap() else {
            panic!("expected ELF binding")
        };
        namespace.bindings.insert(symbol.to_owned(), binding);
    }
    let failure = namespace.project(&BTreeMap::new()).unwrap_err().to_string();
    assert!(
        failure.contains("ELF native symbol") && failure.contains("conflicts"),
        "{failure}"
    );
}
