use super::*;

mod independence;

#[test]
fn actual_hir_type_uses_drive_mir_dependency_projection() {
    let target = resolved_target().expect("supported host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-materialized-type-uses");
    for (name, expected) in [
        // Deferred has a finite coroutine start and an initializer returning Unit.
        ("standalone", vec!["Boolean", "String", "Throwable", "Unit"]),
        ("combined", vec!["Any", "Boolean", "String", "Unit"]),
        ("initialization", vec!["Boolean", "String", "Unit"]),
    ] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        support::with_inspection(
            sysroot.path(),
            &target,
            &bytes,
            &source,
            |input, projection| {
                if name == "standalone" {
                    independence::check(input, projection);
                }
                let uses = projection;
                let mut names = Vec::new();
                for usage in uses {
                    let mir::MirTypeBridgeTargetV1::Type(exact) = usage.target() else {
                        panic!("fixture has only external type requirements")
                    };
                    let key = input
                        .identities
                        .canonical_key::<_, scoop_identity::ExactTypeKey>(exact)
                        .unwrap();
                    let scoop_identity::ExactTypeKey::Nominal(owner) = key.as_ref() else {
                        panic!("parameter-free nominal")
                    };
                    let declaration = input
                        .identities
                        .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*owner)
                        .unwrap();
                    assert_eq!(usage.provider(), declaration.origin());
                    let scoop_identity::DeclarationName::Named(name) = declaration.name() else {
                        panic!("fixture nominal name")
                    };
                    names.push(name.as_str().to_owned());
                }
                names.sort();
                assert_eq!(names, expected, "{name}");
            },
            |_, _| {},
        );
    }
}
