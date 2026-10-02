use super::super::imported_classes::runtime;
use super::*;

#[test]
fn generic_member_templates_republish_and_execute_from_artifacts() {
    check_member_cases(
        &[
            "standalone",
            "method-arguments",
            "signature-support",
            "value-method",
            "plain-owner",
            "inherited",
            "captured",
            "virtual",
            "abi",
            "overloads",
            "interface-class",
            "interface-abstract",
            "interface-struct",
            "interface-enum",
            "interface-local",
            "interface-abi",
            "interface-properties",
            "interface-value-property",
        ],
        &[],
        "downstream",
    );
}

#[test]
fn generic_extension_properties_republish_and_execute_from_artifacts() {
    check_member_cases(
        &[
            "extension-read",
            "extension-write",
            "extension-write-only",
            "extension-abi",
            "extension-inherited",
            "extension-captured",
            "extension-overloads",
            "extension-order",
        ],
        &[
            "extension-readonly",
            "extension-private-setter",
            "extension-rhs-type",
            "extension-result-type",
            "extension-kind-bound",
        ],
        "extension-downstream",
    );
}

#[test]
fn protected_generic_members_republish_and_execute_from_artifacts() {
    check_member_cases(
        &[
            "access-method",
            "access-super",
            "access-setter",
            "access-property-override",
            "access-secondary",
            "access-lexical",
            "access-abi",
            "access-ordinary",
            "access-object",
        ],
        &[
            "access-base-receiver",
            "access-sibling-receiver",
            "access-constructor-error",
            "access-setter-error",
            "access-base-setter",
            "access-private-setter",
            "access-narrow-override",
            "access-public-override",
            "access-narrow-setter",
        ],
        "downstream",
    );
}

fn check_member_cases(cases: &[&str], rejected: &[&str], downstream_source: &str) {
    check_fixture_cases(
        "m23-generic-member-consumption",
        cases,
        rejected,
        downstream_source,
    );
}

pub(super) fn check_fixture_cases(
    fixture: &str,
    cases: &[&str],
    rejected: &[&str],
    downstream_source: &str,
) {
    let target = resolved_target().expect("generic member publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures").join(fixture);
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-member-provider", "0.1.0").unwrap();
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
    let runtime_fixtures = if fixtures.join("runtime.c").is_file() {
        fixtures.clone()
    } else {
        crate::workspace_root().join("tests/fixtures/m23-imported-classes")
    };
    for &case in cases {
        eprintln!("generic member case: {case}");
        let name = format!("generic-member-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &[&provider_coordinate]);
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
                vec![provider.artifact().path().to_path_buf()],
                Vec::new(),
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            let actual = output.emitted_dumps().first().unwrap().text();
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("generic-member-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source(downstream_source),
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
            vec![provider.artifact().path().to_path_buf()],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        let closure = runtime::check(
            &target,
            &[&core, &provider, consumer, &downstream],
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        // Providers can instantiate their own templates and emit value helpers.
        // The runtime closure compares every overlapping ODR member, including
        // definitions shared by the provider and these consumers.
        let mut materializations = 0;
        for artifact in [consumer, &downstream] {
            let identity = artifact
                .artifact()
                .summary()
                .coordinate()
                .identity()
                .unwrap();
            let (sections, _) = closure.artifact(identity).unwrap();
            materializations += sections
                .lir_strong_production()
                .canonical_callable_definitions()
                .definitions()
                .iter()
                .filter(|definition| {
                    matches!(
                        definition.owner(),
                        scoop_lir::CanonicalCallableDefinitionOwnerV1::Odr { .. }
                    )
                })
                .count();
            for binding in sections
                .mir_type_bridge()
                .exports()
                .callables()
                .entries()
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.implementation(),
                        scoop_identity::CallableDefinitionOwner::Odr(_)
                    )
                })
            {
                let abi = sections
                    .lir_exports()
                    .callables()
                    .get(binding.implementation())
                    .expect("a callable has its actual lowered ABI");
                assert_eq!(
                    abi.canonical_signature().signature(),
                    binding.lowered_signature().exact()
                );
                assert_eq!(
                    abi.definition().symbol().linkage(),
                    scoop_identity::LinkageClass::OdrWeak
                );
                assert_eq!(abi.physical_definition().provider(), identity);
            }
        }
        assert!(
            materializations > 0,
            "{case} has actual ODR callable definitions"
        );
        let (sections, _) = closure.artifact(coordinate.identity().unwrap()).unwrap();
        let mut boxes = 0;
        for ty in sections.mir_type_bridge().exports().types().records() {
            let scoop_mir::MirTypeRepresentationV1::BoxedValue { payload } = ty.representation()
            else {
                continue;
            };
            if !matches!(
                sections
                    .identity_graph()
                    .canonical_key::<_, scoop_identity::ExactTypeKey>(payload.value)
                    .unwrap()
                    .as_ref(),
                scoop_identity::ExactTypeKey::NominalApplication { .. }
            ) {
                continue;
            }
            let descriptor = sections
                .lir_exports()
                .descriptors()
                .get(ty.exact())
                .unwrap();
            assert_eq!(
                descriptor.physical_definition().symbol().linkage(),
                scoop_identity::LinkageClass::OdrWeak
            );
            boxes += 1;
        }
        let mut adjusts = 0;
        for binding in sections.mir_type_bridge().exports().callables().entries() {
            if let scoop_mir::MirCallableOriginV1::Generated {
                role: scoop_identity::GeneratedCallableKey::BoxingAdjust { payload, .. },
                ..
            } = binding.origin()
                && matches!(
                    sections
                        .identity_graph()
                        .canonical_key::<_, scoop_identity::ExactTypeKey>(*payload)
                        .unwrap()
                        .as_ref(),
                    scoop_identity::ExactTypeKey::NominalApplication { .. }
                )
            {
                let scoop_identity::CallableDefinitionOwner::Odr(member) = binding.implementation()
                else {
                    panic!("{case}: a generic box adjust retains its ODR owner");
                };
                assert_eq!(
                    member.role(),
                    scoop_identity::OdrMemberRole::DispatchAdapter
                );
                adjusts += 1;
            }
        }
        if matches!(
            case,
            "interface-struct" | "interface-enum" | "interface-abi" | "interface-value-property"
        ) {
            assert!(
                boxes > 0 && adjusts > 0,
                "{case} publishes its actual box and adjust definitions"
            );
        }
    }

    for &case in rejected {
        let root = sysroot.path().join(case);
        let source = source(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        write_dependency_manifest(&root, case, &[&provider_coordinate]);
        let destination = sysroot.path().join(format!("output/{case}.slib"));
        let error = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &destination,
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("{case} must be diagnosed during compilation: {error:?}")
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("{case} must be diagnosed before MIR: {error:?}")
        };
        let actual = diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(diagnostic.file, 0, "{case} belongs to the consumer source");
                let span = diagnostic.span.unwrap();
                format!(
                    "span={}..{}\nexpression={}\n{}\n",
                    span.start,
                    span.end,
                    &source[span.start as usize..span.end as usize],
                    diagnostic.message
                )
            })
            .collect::<String>();
        let snapshot = fixtures.join(format!("{case}.diagnostic.snap"));
        if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap(), "{case}");
        assert!(!destination.exists());
    }
}
