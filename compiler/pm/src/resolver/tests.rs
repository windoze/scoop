use super::*;

fn key(name: &str) -> Key {
    format!("org.example:{name}").parse().unwrap()
}
fn registry() -> RegistryUrl {
    "https://packages.example.org/".parse().unwrap()
}

fn edge(name: &str, version: &str) -> Requirement {
    Requirement {
        key: key(name),
        versions: vec![version.parse().unwrap()],
        source: SourceRequest::Registry(registry()),
        origins: vec![ManifestOrigin {
            manifest: PathBuf::from(format!("{name}/Cone.toml")),
            span: 10..20,
        }],
        test: false,
        artifact: None,
    }
}

fn candidate(name: &str, version: &str, dependencies: Vec<Requirement>) -> Candidate {
    Candidate {
        coordinate: ConeCoordinate::new("org.example", name, version).unwrap(),
        kind: RequestedConeKind::Library,
        source: LockedSource::Registry {
            registry: registry(),
            archive: format!("{name}/{version}.tar.zst"),
            sha256: "aa".repeat(32),
        },
        dependencies,
        yanked: false,
    }
}

fn root(dependencies: Vec<Requirement>) -> Candidate {
    let mut root = candidate("app", "0.1.0", dependencies);
    root.source = LockedSource::Path { path: ".".into() };
    root.kind = RequestedConeKind::Executable;
    root
}

fn run(root: Candidate, candidates: Vec<Candidate>, policy: &ResolvePolicy) -> PmResult<Solution> {
    solve(root, policy, |requirement| {
        Ok(candidates
            .iter()
            .filter(|candidate| {
                candidate.key() == requirement.key
                    && match (&requirement.source, &candidate.source) {
                        (
                            SourceRequest::Registry(a),
                            LockedSource::Registry { registry: b, .. },
                        ) => a == b,
                        (SourceRequest::Path(a), LockedSource::Path { path: b }) => {
                            a == &PathBuf::from(b)
                        }
                        _ => true,
                    }
            })
            .cloned()
            .collect())
    })
}

fn versions(solution: &Solution) -> BTreeMap<String, String> {
    solution
        .packages
        .values()
        .map(|node| {
            (
                node.coordinate.name().into(),
                node.coordinate.version().into(),
            )
        })
        .collect()
}

#[test]
fn backtracks_from_the_latest_candidate_to_satisfy_the_entire_graph() {
    let solution = run(
        root(vec![edge("a", "1.0.0"), edge("b", "1.0.0")]),
        vec![
            candidate("a", "1.2.0", vec![edge("b", "2.0.0")]),
            candidate("a", "1.1.0", vec![edge("b", "1.0.0")]),
            candidate("b", "1.9.0", vec![]),
            candidate("b", "2.0.0", vec![]),
        ],
        &ResolvePolicy::default(),
    )
    .unwrap();
    assert_eq!(versions(&solution)["a"], "1.1.0");
    assert_eq!(versions(&solution)["b"], "1.9.0");
}

#[test]
fn keeps_valid_locks_and_updates_only_requested_preferences() {
    let old = candidate("a", "1.0.0", vec![]);
    let mut policy = ResolvePolicy::default();
    policy.locked.insert(
        old.key(),
        LockedNode {
            coordinate: old.coordinate.clone(),
            kind: old.kind,
            source: old.source.clone(),
            dependencies: Vec::new(),
            test_dependencies: Vec::new(),
        },
    );
    let candidates = vec![old, candidate("a", "1.2.0", vec![])];
    let app = root(vec![edge("a", "1.0.0")]);
    assert_eq!(
        versions(&run(app.clone(), candidates.clone(), &policy).unwrap())["a"],
        "1.0.0"
    );
    policy.update = Some(vec![key("a")]);
    assert_eq!(
        versions(&run(app.clone(), candidates.clone(), &policy).unwrap())["a"],
        "1.2.0"
    );
    policy.precise = Some((key("a"), "2.0.0".into()));
    assert!(run(app, candidates, &policy).is_err());
}

#[test]
fn source_conflicts_are_constraints_and_can_backtrack_an_upstream_version() {
    let other: RegistryUrl = "https://other.example.org/".parse().unwrap();
    let mut b_other = candidate("b", "1.0.0", vec![]);
    b_other.source = LockedSource::Registry {
        registry: other.clone(),
        archive: "b.tar.zst".into(),
        sha256: "bb".repeat(32),
    };
    let mut other_edge = edge("b", "1.0.0");
    other_edge.source = SourceRequest::Registry(other);
    let candidates = vec![
        candidate("a", "1.1.0", vec![other_edge]),
        candidate("a", "1.0.0", vec![edge("b", "1.0.0")]),
        candidate("b", "1.0.0", vec![]),
        b_other,
    ];
    let solution = run(
        root(vec![edge("a", "1.0.0"), edge("b", "1.0.0")]),
        candidates,
        &ResolvePolicy::default(),
    )
    .unwrap();
    assert_eq!(versions(&solution)["a"], "1.0.0");
}

#[test]
fn prereleases_exact_metadata_and_equal_precedence_follow_manifest_rules() {
    let candidates = vec![
        candidate("a", "1.4.0-rc.2", vec![]),
        candidate("a", "1.4.0+z", vec![]),
        candidate("a", "1.4.0+a", vec![]),
        candidate("a", "1.5.0-rc.1", vec![]),
    ];
    let check = |requirement: &str| {
        versions(
            &run(
                root(vec![edge("a", requirement)]),
                candidates.clone(),
                &ResolvePolicy::default(),
            )
            .unwrap(),
        )["a"]
            .clone()
    };
    assert_eq!(check("1.4.0"), "1.4.0+a");
    assert_eq!(check("=1.4.0+z"), "1.4.0+z");
    assert_eq!(check(">=1.4.0-rc.1, <1.4.0"), "1.4.0-rc.2");
    assert!(
        run(
            root(vec![edge("a", "=1.4.0")]),
            candidates,
            &ResolvePolicy::default()
        )
        .is_err()
    );
}

#[test]
fn yanks_allow_existing_locks_but_not_fresh_or_precise_selection() {
    let mut old = candidate("a", "1.0.0", vec![]);
    old.yanked = true;
    let candidates = vec![old.clone()];
    let app = root(vec![edge("a", "1.0.0")]);
    assert!(run(app.clone(), candidates.clone(), &ResolvePolicy::default()).is_err());
    let mut policy = ResolvePolicy::default();
    policy.locked.insert(
        old.key(),
        LockedNode {
            coordinate: old.coordinate,
            kind: old.kind,
            source: old.source,
            dependencies: vec![],
            test_dependencies: vec![],
        },
    );
    assert!(run(app.clone(), candidates.clone(), &policy).is_ok());
    policy.update = Some(vec![key("a")]);
    policy.precise = Some((key("a"), "1.0.0".into()));
    assert!(run(app, candidates, &policy).is_err());
}

#[test]
fn failure_reports_both_dependency_paths_and_manifest_requirements() {
    let error = run(
        root(vec![edge("a", "1.0.0"), edge("b", "1.0.0")]),
        vec![
            candidate("a", "1.0.0", vec![edge("c", "1.0.0")]),
            candidate("b", "1.0.0", vec![edge("c", "2.0.0")]),
            candidate("c", "1.0.0", vec![]),
            candidate("c", "2.0.0", vec![]),
        ],
        &ResolvePolicy::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("org.example:app -> org.example:a -> org.example:c"),
        "{error}"
    );
    assert!(
        error.contains("org.example:app -> org.example:b -> org.example:c"),
        "{error}"
    );
    assert!(error.contains("c/Cone.toml (bytes 10..20)"), "{error}");
    assert!(
        error.contains("requires 1.0.0") && error.contains("requires 2.0.0"),
        "{error}"
    );
}

#[test]
fn independent_roots_and_test_constraints_do_not_force_one_workspace_version() {
    let candidates = vec![
        candidate("a", "1.0.0", vec![]),
        candidate("a", "2.0.0", vec![]),
    ];
    for requirement in ["1.0.0", "2.0.0"] {
        let solution = run(
            root(vec![edge("a", requirement)]),
            candidates.clone(),
            &ResolvePolicy::default(),
        )
        .unwrap();
        assert_eq!(versions(&solution)["a"], requirement);
    }
    let mut test = edge("a", "2.0.0");
    test.test = true;
    assert!(
        run(
            root(vec![edge("a", "1.0.0"), test]),
            candidates,
            &ResolvePolicy::default()
        )
        .is_err()
    );
}

#[test]
fn dependencies_must_be_libraries_and_self_cycles_are_errors() {
    let mut executable = candidate("a", "1.0.0", vec![]);
    executable.kind = RequestedConeKind::Executable;
    assert!(
        run(
            root(vec![edge("a", "1.0.0")]),
            vec![executable],
            &ResolvePolicy::default()
        )
        .is_err()
    );
    let recursive = candidate("a", "1.0.0", vec![edge("a", "1.0.0")]);
    let error = run(
        root(vec![edge("a", "1.0.0")]),
        vec![recursive],
        &ResolvePolicy::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("depends on itself"), "{error}");
}

#[test]
fn implicit_core_uses_the_explicit_transitive_choice_without_loading_the_sysroot() {
    let core = Candidate {
        coordinate: ConeCoordinate::reserved_core(),
        kind: RequestedConeKind::Library,
        source: LockedSource::Path {
            path: "../core".into(),
        },
        dependencies: vec![],
        yanked: false,
    };
    let implicit = Requirement {
        key: core.key(),
        versions: vec!["=0.1.0".parse().unwrap()],
        source: SourceRequest::ImplicitCore,
        origins: vec![],
        test: false,
        artifact: None,
    };
    let mut explicit = implicit.clone();
    explicit.source = SourceRequest::Path("../core".into());
    let a = candidate("a", "1.0.0", vec![explicit]);
    let solution = solve(
        root(vec![edge("a", "1.0.0"), implicit]),
        &ResolvePolicy::default(),
        |edge| {
            assert!(!matches!(
                edge.source,
                SourceRequest::ImplicitCore | SourceRequest::System
            ));
            Ok(vec![if edge.key == core.key() {
                core.clone()
            } else {
                a.clone()
            }])
        },
    )
    .unwrap();
    assert_eq!(solution.packages[&core.key()].source, core.source);
    solution
        .lock(
            ".".into(),
            Purpose::Production,
            "aarch64-apple-darwin".into(),
        )
        .unwrap();
}
