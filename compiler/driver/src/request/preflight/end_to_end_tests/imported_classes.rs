use super::*;

mod runtime;

#[test]
fn dependency_classes_compile_and_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "cast-class",
            "cast-failure",
            "cast-interface",
            "cast-zst",
            "cast-wide",
            "cast-default",
            "cast-default-failure",
            "exception-statement",
            "exception-expression",
            "exception-default",
            "standalone",
            "combined",
            "aliases",
            "secondary",
            "recursive",
            "core-extension",
            "virtual",
            "interface",
            "interface-alias",
            "interface-parent",
            "reference-identity",
            "core-interface",
            "interface-values",
            "interface-default-constructor",
        ],
        &[
            "cast-impossible",
            "wrong-identity",
            "no-gc",
            "internal-constructor",
            "abstract-constructor",
            "interface-wrong-identity",
            "interface-wrong-argument",
            "throw-wrong-type",
            "throw-wrong-identity",
            "catch-wrong-type",
            "catch-expression-wrong-type",
            "catch-unreachable",
        ],
    );
}

#[test]
fn dependency_property_writes_compile_and_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "property-setter",
            "property-computed",
            "property-virtual",
            "property-interface",
            "property-updates",
            "property-abi",
        ],
        &[
            "property-readonly",
            "property-private-setter",
            "property-internal-setter",
            "property-private-update",
            "property-wrong-value",
            "property-wrong-reference",
        ],
    );
}

#[test]
fn dependency_value_interfaces_compile_and_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "value-interface-struct",
            "value-interface-enum",
            "value-interface-zst",
            "value-interface-wide",
            "value-interface-reference",
            "value-interface-recursive",
            "value-interface-default",
            "value-interface-core",
        ],
        &[
            "value-interface-wrong-identity",
            "enum-interface-wrong-identity",
        ],
    );
}

#[test]
fn primitive_interfaces_compile_and_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "primitive-any",
            "primitive-boolean",
            "primitive-integers",
            "primitive-hash",
            "primitive-string",
            "primitive-default",
            "primitive-core",
            "primitive-cast-failure",
        ],
        &[
            "primitive-wrong-interface",
            "boolean-wrong-interface",
            "primitive-variance",
        ],
    );
}

#[test]
fn integer_division_compiles_and_runs_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "arithmetic-direct",
            "arithmetic-widths",
            "arithmetic-zero",
            "arithmetic-default",
            "arithmetic-core",
        ],
        &["arithmetic-no-gc", "arithmetic-wrong-argument"],
    );
}

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

fn check_class_cases(cast_variant: &str, cases: &[&str], negative_cases: &[&str]) {
    let target =
        resolved_target().expect("the production test requires the configured LLVM target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let original_fingerprint = core.artifact().summary().artifact_fingerprint();
    let core_source = sysroot.path().join("editable-core");
    std::fs::rename(sysroot.path().join("lib/scoop.core"), &core_source).unwrap();
    std::fs::write(
        core_source.join("src/user_class.scoop"),
        source("core-provider"),
    )
    .unwrap();
    let types = core_source.join("src/types.scoop");
    let original_types = std::fs::read_to_string(&types).unwrap();
    let changed_types = original_types.replace(
        "public struct Long : ToString, Hash {",
        "public struct Long : ToString, Hash, RebuiltReadable {\n    public override fun read(): Long = this + stage3CoreAnswer() - 43",
    );
    assert_ne!(original_types, changed_types);
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
    std::fs::write(throwable, changed_arithmetic).unwrap();
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
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "class-provider",
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
            request.emit = StageDumpPolicy::Stage(kind);
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
                output.emitted_dump().unwrap().text(),
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
            &source("downstream"),
        );
        write_dependency_manifest(&root, &name, &[&coordinate, &consumer]);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![
                provider.artifact().path().to_path_buf(),
                output.artifact().path().to_path_buf(),
            ],
            vec![],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("downstream {case}: {error:?}"));
        runtime::check(
            &target,
            &[&core, &provider, output, &downstream],
            &runtime,
            &fixtures,
            &sysroot.path().join(format!("run-{case}")),
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
            panic!("class errors must be diagnosed by the HIR stage");
        };
        if *case == "primitive-variance" {
            assert!(matches!(
                error.cause(),
                CurrentConeProductionFailure::Mir(CurrentConeMirStageError::Foundation(
                    scoop_mir::OdrFreeMirFoundationProjectionError::Odr(
                        scoop_mir::OdrFreeMirFoundationError::CallableSignatureSubject(_)
                    )
                ))
            ));
            snapshot(
                &fixtures.join(format!("{case}.diagnostic.snap")),
                &format!("{}\n", error.cause()),
            );
            assert!(!destination.exists());
            continue;
        }
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
