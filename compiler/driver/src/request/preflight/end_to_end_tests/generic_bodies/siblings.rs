use std::collections::BTreeMap;

use super::super::imported_classes::runtime;
use super::*;

mod conflicts;

#[test]
fn sibling_generic_instances_merge_through_artifacts_and_run_with_moving_gc() {
    check_siblings(&["standalone", "combined"]);
}

#[test]
fn generic_string_constants_merge_through_artifacts_and_preserve_identity() {
    check_siblings(&["strings", "strings-combined"]);
}

fn check_siblings(cases: &[&str]) {
    let target = resolved_target().expect("sibling generic publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-odr-siblings");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-odr-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &source("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    std::fs::rename(
        provider_root.join("src"),
        provider_root.join("unused-source"),
    )
    .unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let runtime_fixture = crate::workspace_root().join("tests/fixtures/m23-imported-classes");

    for &case in cases {
        let mut siblings = Vec::new();
        let mut coordinates = Vec::new();
        for side in ["left", "right"] {
            let name = format!("generic-odr-{case}-{side}");
            let root = sysroot.path().join(&name);
            write_manifest_cone(
                &root,
                "dev.example",
                &name,
                "library",
                &source(&format!("{case}-{side}")),
            );
            write_dependency_manifest(&root, &name, &[&provider_coordinate]);
            let request = || {
                build_manifest_request(
                    sysroot.path(),
                    &target,
                    &root,
                    &sysroot.path().join(format!("output/{name}.slib")),
                    vec![provider.artifact().path().to_path_buf()],
                    Vec::new(),
                )
            };
            if case.starts_with("strings") {
                for (kind, stage) in [(StageDumpKind::Hir, "hir"), (StageDumpKind::Mir, "mir")] {
                    let mut request = request();
                    request.emit = StageDumpPolicy::Stage(kind);
                    let output = request
                        .build_and_publish()
                        .unwrap_or_else(|error| panic!("{case} {side} {stage}: {error:?}"));
                    check_string_snapshot(
                        &fixtures,
                        case,
                        side,
                        stage,
                        output.emitted_dump().unwrap().text(),
                    );
                }
            }
            let mut request = request();
            if case.starts_with("strings") {
                request.emit = StageDumpPolicy::Stage(StageDumpKind::Lir);
            }
            let artifact = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {side}: {error:?}"));
            if case.starts_with("strings") {
                check_string_snapshot(
                    &fixtures,
                    case,
                    side,
                    "lir",
                    artifact.emitted_dump().unwrap().text(),
                );
            }
            std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
            coordinates.push(ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap());
            siblings.push(artifact);
        }
        let name = format!("generic-odr-{case}-consumer");
        let root = sysroot.path().join(&name);
        write_manifest_cone(
            &root,
            "dev.example",
            &name,
            "library",
            &source(&format!("{case}-consumer")),
        );
        write_dependency_manifest(
            &root,
            &name,
            &[&provider_coordinate, &coordinates[0], &coordinates[1]],
        );
        let consumer = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![
                provider.artifact().path().to_path_buf(),
                siblings[0].artifact().path().to_path_buf(),
                siblings[1].artifact().path().to_path_buf(),
            ],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} consumer: {error:?}"));
        let closure = runtime::check(
            &target,
            &[&core, &provider, &siblings[0], &siblings[1], &consumer],
            &runtime,
            &runtime_fixture,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        let mut expected = BTreeMap::<_, Vec<_>>::new();
        for (_, artifact) in closure.dependency_first() {
            for group in artifact.production_projection().odr_members().groups() {
                for member in group.members() {
                    expected
                        .entry((group.group(), member.member()))
                        .or_default()
                        .push(artifact.provider());
                }
            }
        }
        let merged = closure.odr_definitions();
        assert_eq!(merged.members().len(), expected.len());
        let reversed = scoop_slib::merge_cross_cone_odr_definitions(
            closure
                .dependency_first()
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        )
        .unwrap();
        assert_eq!(reversed.members().len(), expected.len());
        let left = coordinates[0].identity().unwrap();
        let right = coordinates[1].identity().unwrap();
        let mut shared_bodies = 0;
        let mut shared_registrations = 0;
        let mut shared_immortals = 0;
        for ((group, member), providers) in expected {
            let definition = merged.get(group, member).unwrap();
            let reverse = reversed.get(group, member).unwrap();
            assert_eq!(reverse.group_key(), definition.group_key());
            assert_eq!(reverse.key(), definition.key());
            assert_eq!(reverse.abi(), definition.abi());
            assert_eq!(reverse.definition(), definition.definition());
            assert_eq!(
                reverse
                    .candidates()
                    .map(|candidate| candidate.provider())
                    .collect::<Vec<_>>(),
                providers.iter().copied().rev().collect::<Vec<_>>(),
            );
            assert_eq!(definition.member(), member);
            assert_eq!(definition.key().group(), group);
            assert_eq!(
                definition
                    .candidates()
                    .map(|candidate| candidate.provider())
                    .collect::<Vec<_>>(),
                providers
            );
            for candidate in definition.candidates() {
                assert_eq!(
                    candidate.definition().owner(),
                    scoop_slib::LinkDefinitionOwnerV1::OdrDefinition(member)
                );
                let (_, artifact) = closure.artifact(candidate.provider()).unwrap();
                assert!(
                    artifact
                        .defined_symbols()
                        .owners()
                        .contains(candidate.definition())
                );
            }
            if providers.contains(&left) && providers.contains(&right) {
                shared_bodies += usize::from(
                    definition.key().role() == scoop_identity::OdrMemberRole::CallableBody,
                );
                shared_registrations += usize::from(
                    definition.key().role() == scoop_identity::OdrMemberRole::RegistrationRecord,
                );
                shared_immortals += usize::from(
                    definition.key().role() == scoop_identity::OdrMemberRole::ImmortalObject,
                );
            }
        }
        assert!(shared_bodies >= 2, "{case}: {shared_bodies}");
        assert!(shared_registrations >= 2, "{case}: {shared_registrations}");
        if case.starts_with("strings") {
            assert!(shared_immortals >= 4, "{case}: {shared_immortals}");
        }
    }
}

fn check_string_snapshot(
    fixtures: &std::path::Path,
    case: &str,
    side: &str,
    stage: &str,
    actual: &str,
) {
    let path = fixtures.join(format!("{case}-{side}.{stage}.snap"));
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
