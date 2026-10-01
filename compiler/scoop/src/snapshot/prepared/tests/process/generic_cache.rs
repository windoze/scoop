use super::*;

#[test]
fn generic_dependency_changes_rebuild_consumers_and_unchanged_inputs_hit_cache() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let dependency = workspace.join("dependency");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    let provider_source =
        include_str!("../../../../../../../tests/fixtures/m23-generic-cache/provider.scoop");
    let consumer_source =
        include_str!("../../../../../../../tests/fixtures/m23-generic-cache/consumer.scoop");
    write_manifest_source(&dependency, "dependency", "library", provider_source, "");
    write_manifest_source(
        &root,
        "root",
        "library",
        consumer_source,
        "[dependencies]\n\"test:dependency\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
    );
    let dependency_identity = ConeCoordinate::new("test", "dependency", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let identities = [ConeIdentity::CORE, dependency_identity, root_identity];
    let build = || {
        real_manifest_request(&root, workspace, &sysroot, &compiler)
            .load_root()
            .unwrap()
            .discover()
            .unwrap()
            .resolve()
            .unwrap()
            .prepare()
            .unwrap()
            .execute()
            .unwrap()
    };
    let mut previous = build();
    assert_eq!(previous.observations().child_invocations(), &identities);
    assert_cached(&previous, &build());

    let body = provider_source.replace("= hidden(value, amount)", "= hidden(value, amount) + 1");
    let helper = body.replace("= amount + 1", "= amount + 2");
    let default = helper.replace("amount: Int = 41", "amount: Int = 42");
    let condition = default.replace("fun <T> calculate", "fun <T : ref> calculate");
    let arguments = consumer_source.replace("calculate(\"seed\")", "calculate(Payload(1))");
    for (name, provider, consumer, expected) in [
        ("body", body.as_str(), consumer_source, &identities[1..]),
        (
            "hidden helper",
            helper.as_str(),
            consumer_source,
            &identities[1..],
        ),
        (
            "default",
            default.as_str(),
            consumer_source,
            &identities[1..],
        ),
        (
            "condition",
            condition.as_str(),
            consumer_source,
            &identities[1..],
        ),
        (
            "type arguments",
            condition.as_str(),
            arguments.as_str(),
            &identities[2..],
        ),
    ] {
        eprintln!("generic cache case: {name}");
        std::fs::write(dependency.join("src/main.scoop"), provider).unwrap();
        std::fs::write(root.join("src/main.scoop"), consumer).unwrap();
        let changed = build();
        assert_eq!(
            changed.observations().child_invocations(),
            expected,
            "{name}"
        );
        for identity in identities {
            let before = previous.completed(identity).unwrap();
            let after = changed.completed(identity).unwrap();
            if expected.contains(&identity) {
                assert_eq!(after.origin(), CompletedNodeOrigin::Compiled, "{name}");
                assert_ne!(
                    before.artifact().summary().artifact_fingerprint(),
                    after.artifact().summary().artifact_fingerprint(),
                    "{name}: changed compile inputs are reflected in the artifact",
                );
            } else {
                assert_eq!(after.origin(), CompletedNodeOrigin::CacheHit, "{name}");
                assert_eq!(
                    before.artifact().snapshot().as_bytes(),
                    after.artifact().snapshot().as_bytes(),
                    "{name}",
                );
            }
        }
        assert_cached(&changed, &build());
        previous = changed;
    }
}

fn assert_cached(expected: &crate::ExecutedBuildGraph, actual: &crate::ExecutedBuildGraph) {
    assert!(actual.observations().child_invocations().is_empty());
    for &identity in expected.dependency_first() {
        let before = expected.completed(identity).unwrap();
        let after = actual.completed(identity).unwrap();
        assert_eq!(after.origin(), CompletedNodeOrigin::CacheHit);
        assert_eq!(
            before.artifact().snapshot().as_bytes(),
            after.artifact().snapshot().as_bytes(),
        );
    }
}
