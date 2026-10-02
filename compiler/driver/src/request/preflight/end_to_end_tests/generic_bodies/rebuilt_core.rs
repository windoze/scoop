use super::super::imported_classes::runtime;
use super::*;

#[test]
fn modified_core_array_bodies_and_new_generic_templates_republish_and_execute() {
    let target = resolved_target().expect("rebuilt core generic publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let original = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-rebuilt-core-generics");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let core_root = sysroot.path().join("lib/scoop.core");
    let types_path = core_root.join("src/types.scoop");
    let mut types = std::fs::read_to_string(&types_path).unwrap();
    for iterator in ["ArrayIterator", "MutableArrayIterator"] {
        let original = format!("= {iterator}<T>(this, 0L)");
        assert_eq!(types.matches(&original).count(), 1);
        types = types.replace(&original, &format!("= {iterator}<T>(this, 1L)"));
    }
    std::fs::write(&types_path, types).unwrap();
    std::fs::write(
        core_root.join("src/rebuilt_generics.scoop"),
        source("core-additions"),
    )
    .unwrap();
    let core = crate::normalize_direct_build_request(
        &core_root,
        vec![],
        vec![],
        original.artifact().path(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap();
    assert_ne!(
        original.artifact().summary().artifact_fingerprint(),
        core.artifact().summary().artifact_fingerprint()
    );
    std::fs::rename(core_root.join("src"), core_root.join("unused-source")).unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));

    for case in ["standalone", "combined"] {
        let name = format!("rebuilt-core-array-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        let mut outputs = Vec::new();
        for (kind, stage) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let mut request = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join(format!("output/{name}.slib")),
                vec![],
                vec![],
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let actual = output.emitted_dumps().first().unwrap().text();
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("{name}-downstream");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source("downstream"),
        );
        write_dependency_manifest(&downstream_root, &downstream_name, &[&coordinate]);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![consumer.artifact().path().to_path_buf()],
            vec![],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        runtime::check(
            &target,
            &[&core, consumer, &downstream],
            &runtime,
            &crate::workspace_root().join("tests/fixtures/m23-imported-classes"),
            &sysroot.path().join(format!("run-{case}")),
            &name,
        );
    }
}
