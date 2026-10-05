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
        .join("../../tests/fixtures/m23-native-link/resolution/unused.c");
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
    resolve(&mut inputs, &declarations).unwrap();
    assert!(inputs.native.selected.is_empty());

    inputs.requirements.insert(inputs.symbol("m23_final"));
    resolve(&mut inputs, &declarations).unwrap();
    assert_eq!(inputs.native.selected.len(), 1);
    assert_eq!(
        inputs.native.references.len(),
        usize::from(matches!(
            fixture.profile,
            ValidatedFinalLinkProfile::Darwin(_)
        ))
    );
    resolve(&mut inputs, &declarations).unwrap();
    assert_eq!(inputs.native.selected.len(), 1);
    let objects = inputs.native.ordered_files()[0].objects();
    assert_eq!(objects.len(), 2);
    assert!(inputs.native.selected.contains_key(&objects[0].0));
    assert!(!inputs.native.selected.contains_key(&objects[1].0));
}
