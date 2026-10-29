use super::*;
use crate::test_support;
use std::{path::Path, process::Command};

#[test]
fn empty_worklist_extracts_nothing_and_later_work_selects_each_member_once() {
    let fixture = test_support::native_fixture();
    let path = fixture.directory.path();
    let mut inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
    )
    .unwrap();
    let member = path.join("member.o");
    std::fs::rename(path.join("m23_final.o"), &member).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/language/native-link/resolution/unused.c");
    let unused = path.join("unused.o");
    test_support::compile_native(&fixture.profile, &source, &unused);
    let status = Command::new("/usr/bin/ar")
        .arg("qcS")
        .arg(path.join("libm23_final.a"))
        .args([&member, &unused])
        .status()
        .unwrap();
    assert!(status.success());

    let (declarations, libraries) = declarations::read(&fixture.closure).unwrap();
    inputs.native =
        NativeInputs::read(libraries, &fixture.library_paths, &fixture.profile).unwrap();
    inputs
        .definitions
        .retain(|_, owner| !matches!(owner, DefinitionOwner::Native(_)));
    inputs.requirements.clear();
    for symbol in inputs
        .namespace
        .bound_symbols()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
    {
        inputs.namespace.unbind(&symbol);
    }
    resolve(&mut inputs, &declarations, &fixture.runtime).unwrap();
    assert!(inputs.native.selected.is_empty());

    inputs.requirements.insert(inputs.symbol("m23_final"));
    resolve(&mut inputs, &declarations, &fixture.runtime).unwrap();
    assert_eq!(inputs.native.selected.len(), 1);
    resolve(&mut inputs, &declarations, &fixture.runtime).unwrap();
    assert_eq!(inputs.native.selected.len(), 1);
    let objects = inputs.native.ordered_files()[0].objects();
    assert_eq!(objects.len(), 2);
    assert!(inputs.native.selected.contains_key(&objects[0].0));
    assert!(!inputs.native.selected.contains_key(&objects[1].0));
}

#[test]
#[cfg(target_os = "linux")]
fn overlapping_elf_exports_follow_the_final_dynamic_library_order() {
    use crate::namespace::{NativeBinding, NativeNamespace, elf::ElfBinding};
    use scoop_identity::{
        CanonicalNativeLibraryName, NativeLibraryGrouping, NativeLibraryKind,
        NativeLinkRequirementId, NativeLinkRequirementKey,
    };

    let fixture = test_support::native_fixture();
    let path = fixture.directory.path();
    let mut libraries = BTreeMap::new();
    for (name, value) in [("first", 11), ("second", 21)] {
        let source = path.join(format!("{name}.c"));
        std::fs::write(
            &source,
            format!(
                "int overlap(void) {{ return {value}; }}\nint {name}_only(void) {{ return 1; }}\n"
            ),
        )
        .unwrap();
        let status = fixture
            .profile
            .startup_toolchain()
            .driver_command()
            .args(["-shared", "-fPIC"])
            .arg(source)
            .arg("-o")
            .arg(path.join(format!("lib{name}.so")))
            .status()
            .unwrap();
        assert!(status.success());
        let key = NativeLinkRequirementKey::for_target(
            fixture.profile.target().wire_id(),
            CanonicalNativeLibraryName::new(name).unwrap(),
            NativeLibraryKind::Dynamic,
            NativeLibraryGrouping::Independent,
        );
        libraries.insert(
            NativeLinkRequirementId::from_key(&key).unwrap(),
            (key, vec![name.to_owned()]),
        );
    }
    let mut inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
    )
    .unwrap();
    inputs.native =
        NativeInputs::read(libraries, &fixture.library_paths, &fixture.profile).unwrap();
    inputs.namespace =
        NativeNamespace::read(&mut inputs.native, &fixture.library_paths, &fixture.profile)
            .unwrap();
    for symbol in ["overlap", "first_only", "second_only"] {
        let Candidate::Dynamic(binding) = candidate(
            symbol,
            NativeLibraryBinding::DefaultNativeNamespace,
            &inputs,
        )
        .unwrap() else {
            panic!("expected a shared library export");
        };
        inputs.namespace.bind(symbol.to_owned(), binding).unwrap();
    }
    inputs
        .namespace
        .project(
            &inputs.definitions,
            &fixture.library_paths,
            &fixture.profile,
        )
        .unwrap();
    let NativeNamespace::Elf(namespace) = &mut inputs.namespace else {
        panic!("expected ELF namespace");
    };
    let ElfBinding::Shared { input, .. } = &namespace.bindings["overlap"] else {
        panic!("expected shared export");
    };
    let order = namespace.selected_order();
    assert_eq!(order.len(), 2);
    assert_eq!(*input, order[0]);
    let expected = if namespace.providers[input].name == "libfirst.so" {
        11
    } else {
        21
    };
    let source = path.join("overlap-main.c");
    std::fs::write(&source, format!(
        "int overlap(void); int first_only(void); int second_only(void);\nint main(void) {{ return overlap() != {expected} || first_only() + second_only() != 2; }}\n"
    )).unwrap();
    let output = path.join("overlap-program");
    let mut command = fixture.profile.startup_toolchain().driver_command();
    command
        .arg(source)
        .arg("-o")
        .arg(&output)
        .arg(format!("-Wl,-rpath,{}", path.display()));
    for id in order {
        command.arg(&namespace.providers[&id].locator);
    }
    assert!(command.status().unwrap().success());
    assert!(Command::new(output).status().unwrap().success());

    // Explicit library contracts still cannot request a later interposed definition.
    let explicit = inputs
        .native
        .libraries
        .values()
        .find(|library| library.inputs[0] != *input)
        .unwrap();
    let NativeBinding::Elf(binding) = namespace
        .candidates("overlap", Some(&explicit.inputs), false)
        .pop()
        .unwrap()
    else {
        panic!("expected ELF export");
    };
    namespace.bindings.insert("overlap".to_owned(), binding);
    let failure = namespace
        .project(&inputs.definitions)
        .unwrap_err()
        .to_string();
    assert!(failure.contains("is searched before required"), "{failure}");
}
