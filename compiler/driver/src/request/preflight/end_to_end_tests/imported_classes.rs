use super::*;

mod conformance;
mod inheritance;
pub(super) mod runtime;

#[test]
fn runtime_arithmetic_failures_use_the_providers_default_constructor_adapter() {
    check_class_cases("default", &["arithmetic-zero", "arithmetic-direct"], &[]);
}

#[test]
fn runtime_cast_failures_use_the_providers_default_constructor_adapter() {
    check_class_cases(
        "default",
        &["cast-failure", "cast-default-failure", "cast-zst"],
        &[],
    );
}

#[test]
fn dependency_singletons_initialize_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "initialization-object-value",
            "initialization-imported-object",
            "initialization-object-default",
            "initialization-object-exported-default",
            "initialization-object-dispatch",
            "initialization-object-zst",
            "initialization-object-reexport",
        ],
        &[
            "initialization-object-no-gc",
            "initialization-object-wrong-identity",
            "initialization-object-internal",
        ],
    );
}

#[test]
fn dependency_companions_compile_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "companion-imported",
            "companion-named",
            "companion-alias",
            "companion-forwarded",
            "companion-call-import",
            "companion-exported-default",
            "companion-reexport",
            "companion-nested",
            "companion-const",
            "companion-dispatch",
            "companion-write",
            "companion-updates",
            "companion-imported-update",
            "companion-write-order",
            "companion-imported-write-order",
        ],
        &[
            "companion-internal",
            "companion-wrong-identity",
            "companion-no-gc",
            "companion-instance",
            "companion-constructor",
            "companion-readonly",
            "companion-const-write",
            "companion-private-setter",
        ],
    );
}

fn check_class_cases(cast_variant: &str, cases: &[&str], negative_cases: &[&str]) {
    let target =
        resolved_target().expect("the production test requires the configured LLVM target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let original_fingerprint = core.artifact().summary().artifact_fingerprint();
    let intrinsic_defaults = cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("primitive-inherited-"));
    let inheritance = cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("inheritance-"));
    let core_source = sysroot.path().join("editable-core");
    std::fs::rename(sysroot.path().join("lib/scoop.core"), &core_source).unwrap();
    let mut core_provider = source("core-provider");
    if intrinsic_defaults {
        core_provider.push_str(&source("core-member-defaults"));
    }
    if inheritance {
        core_provider.push_str(&source("core-inheritance-provider"));
    }
    std::fs::write(core_source.join("src/user_class.scoop"), core_provider).unwrap();
    let types = core_source.join("src/types.scoop");
    let original_types = std::fs::read_to_string(&types).unwrap();
    let mut changed_types = original_types.replace(
        "public struct Long : ToString, Hash {",
        "public struct Long : ToString, Hash, RebuiltReadable {\n    public override fun read(): Long = this + stage3CoreAnswer() - 43",
    );
    assert_ne!(original_types, changed_types);
    if intrinsic_defaults {
        for declaration in [
            "public struct Long : ToString, Hash, RebuiltReadable",
            "public struct Boolean : ToString, Hash",
            "public class String : ToString, Hash",
        ] {
            let before = format!("{declaration} {{");
            let after = format!("{declaration}, RebuiltDefaults {{");
            assert!(changed_types.contains(&before));
            changed_types = changed_types.replace(&before, &after);
        }
    }
    std::fs::write(types, changed_types).unwrap();
    let throwable = core_source.join("src/throwable.scoop");
    let original = std::fs::read_to_string(&throwable).unwrap();
    let replacement = source(&format!("core-cast-{cast_variant}"));
    let changed_exception = original.replace(
        "public class ClassCastException public constructor() : Exception(Some(\"invalid cast\"))",
        replacement.trim(),
    );
    assert_ne!(original, changed_exception);
    let replacement = source(&format!("core-arithmetic-{cast_variant}"));
    let changed_arithmetic = changed_exception.replace(
        "public class ArithmeticException public constructor() : Exception(Some(\"arithmetic error\"))",
        replacement.trim(),
    );
    assert_ne!(changed_exception, changed_arithmetic);
    let changed_services = if cases.iter().any(|case| case.starts_with("initialization-")) {
        let changed = changed_arithmetic.replace(
            "internal fun __scoopThrowInitializationCycle(message: String) {\n    throw IllegalStateException(Some(message))\n}",
            source("core-initialization").trim(),
        );
        assert_ne!(changed_arithmetic, changed);
        changed
    } else {
        changed_arithmetic
    };
    std::fs::write(throwable, changed_services).unwrap();
    let changed = core_source.join("src/stage3_test.scoop");
    std::fs::write(
        &changed,
        std::fs::read_to_string(&changed)
            .unwrap()
            .replace("= 42", "= 43"),
    )
    .unwrap();
    let core = crate::normalize_direct_build_request(
        &core_source,
        vec![],
        vec![],
        core.artifact().path(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
        Default::default(),
    )
    .unwrap()
    .build_and_publish()
    .unwrap_or_else(|error| match error {
        SingleConeProductionError::Production(error) => {
            panic!("modified core: {:?}", error.cause())
        }
        error => panic!("modified core: {error:?}"),
    });
    assert_ne!(
        original_fingerprint,
        core.artifact().summary().artifact_fingerprint()
    );
    std::fs::rename(core_source.join("src"), core_source.join("unused-source")).unwrap();
    let provider_root = sysroot.path().join("provider");
    let coordinate = ConeCoordinate::new("dev.example", "class-provider", "0.1.0").unwrap();
    let mut provider_source = source("provider");
    if inheritance {
        provider_source.push_str(&source("inheritance-provider"));
    }
    if cases
        .iter()
        .any(|case| case.starts_with("inheritance-abi-"))
    {
        provider_source.push_str(&source("inheritance-abi-provider"));
    }
    if cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("inheritance-access-"))
    {
        provider_source.push_str(&source("inheritance-access-provider"));
    }
    if cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("conformance-"))
    {
        provider_source.push_str(&source("conformance-provider"));
    }
    if cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("conformance-super-"))
    {
        provider_source.push_str(&source("super-provider"));
    }
    if cases
        .iter()
        .chain(negative_cases)
        .any(|case| case.starts_with("conformance-parameter-"))
    {
        provider_source.push_str(&source("parameter-provider"));
    }
    if cases.iter().any(|case| {
        case.starts_with("initialization-object-") || *case == "initialization-imported-object"
    }) {
        provider_source.push_str(&source("initialization-provider"));
    }
    if cases.iter().any(|case| case.starts_with("companion-")) {
        provider_source.push_str(&source("companion-provider"));
    }
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "class-provider",
        "library",
        &provider_source,
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
    for case in cases {
        let name = format!("classes-{case}");
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &[&coordinate]);
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
                vec![],
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot_case = if cast_variant == "direct" {
                case.to_string()
            } else {
                format!("{case}-default-constructor")
            };
            snapshot(
                &fixtures.join(format!("{snapshot_case}.{stage}.snap")),
                output.emitted_dumps().first().unwrap().text(),
            );
            outputs.push(output);
        }
        let output = outputs.last().unwrap();
        let name = format!("class-downstream-{case}");
        let root = sysroot.path().join(&name);
        let consumer =
            ConeCoordinate::new("dev.example", &format!("classes-{case}"), "0.1.0").unwrap();
        write_manifest_cone(
            &root,
            "dev.example",
            &name,
            "library",
            &source(if *case == "inheritance-published" {
                "inheritance-downstream"
            } else if *case == "inheritance-abi-wide" {
                "inheritance-abi-downstream"
            } else if *case == "initialization-object-reexport" {
                "initialization-object-downstream"
            } else if *case == "companion-reexport" {
                "companion-downstream"
            } else if matches!(
                *case,
                "conformance-interface" | "conformance-interface-overload"
            ) {
                "conformance-interface-downstream"
            } else if *case == "conformance-interface-diamond" {
                "conformance-interface-diamond-downstream"
            } else if *case == "conformance-interface-property" {
                "conformance-interface-property-downstream"
            } else if *case == "conformance-super-default" {
                "conformance-super-default-downstream"
            } else if *case == "conformance-parameter-published" {
                "conformance-parameter-downstream"
            } else if *case == "conformance-abstract-method" {
                "conformance-abstract-method-downstream"
            } else if matches!(
                *case,
                "conformance-abstract-default" | "conformance-abstract-interface"
            ) {
                "conformance-abstract-choice-downstream"
            } else if *case == "conformance-abstract-property" {
                "conformance-abstract-property-downstream"
            } else if *case == "conformance-abstract-abi" {
                "conformance-abstract-abi-downstream"
            } else {
                "downstream"
            }),
        );
        if *case == "companion-reexport" {
            write_dependency_manifest(&root, &name, &[&consumer]);
        } else {
            write_dependency_manifest(&root, &name, &[&coordinate, &consumer]);
        }
        let (direct, support) = if *case == "companion-reexport" {
            (
                vec![output.artifact().path().to_path_buf()],
                vec![provider.artifact().path().to_path_buf()],
            )
        } else {
            (
                vec![
                    provider.artifact().path().to_path_buf(),
                    output.artifact().path().to_path_buf(),
                ],
                vec![],
            )
        };
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            direct,
            support,
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("downstream {case}: {error:?}"));
        runtime::check(
            &target,
            &[&core, &provider, output, &downstream],
            &runtime,
            &fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
    }
    for case in negative_cases {
        let root = sysroot.path().join(case);
        let source = source(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        write_dependency_manifest(&root, case, &[&coordinate]);
        let destination = sysroot.path().join(format!("output/{case}.slib"));
        let error = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &destination,
            vec![provider.artifact().path().to_path_buf()],
            vec![],
        )
        .build_and_publish()
        .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("{case}: expected HIR diagnostics, got {error:?}");
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("class errors must precede MIR: {error:?}");
        };
        let text = diagnostics
            .iter()
            .map(|diagnostic| {
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
        snapshot(&fixtures.join(format!("{case}.diagnostic.snap")), &text);
        assert!(!destination.exists());
    }
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_IMPORTED_CLASSES").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        text,
        "{}",
        path.display()
    );
}
