use super::*;
use crate::{DocumentationOutputDestination, SingleConeOutput};

const SOURCE: &str = "package test\r\n\
                      /// 文档 [Missing].\r\n\
                      @DocAlias(\"result\")\r\n\
                      public fun answer(): Long = 42\r\n";

#[test]
fn documentation_pipeline_checks_bodies_and_shares_hir_with_artifact_production() {
    let target = resolved_target().expect("artifact integration tests require a host target");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let root = workspace.path().join("library");
    write_manifest_cone(&root, "test", "documentation", "library", SOURCE);
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/excluded.scoop"), "invalid test source").unwrap();
    let input_path = workspace.path().join("library.json");
    let artifact_path = workspace.path().join("library.slib");
    let make_request = || {
        SingleConeBuildRequest::documentation(
            crate::request::test_support::expanded_fixture(ManifestRootLocator::cone_directory(
                &root,
            )),
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
            TrustedCoreInput::Artifact(HostArtifactLocator::new(core.artifact().path()).unwrap()),
            target.lir_target_selection(),
            DocumentationOutputDestination::new(&input_path).unwrap(),
            DiagnosticOutputPolicy::Structured,
            StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(StageDumpKind::Hir)),
        )
        .unwrap()
    };
    assert!(matches!(
        make_request().build_and_publish(),
        Err(SingleConeProductionError::ArtifactOutputRequired)
    ));
    assert!(!input_path.exists());
    assert!(matches!(
        make_request().compile(true),
        Err(SingleConeProductionError::DocumentationObservation)
    ));

    let result = make_request().compile(false).unwrap();
    assert!(result.warnings().is_empty());
    assert_eq!(result.emitted_dumps().len(), 1);
    let SingleConeCompilation::Documented(documented) = result else {
        panic!("documentation-only must not publish a machine artifact");
    };
    let documentation = documented.documentation();
    assert_eq!(documentation.path(), input_path);
    assert!(!artifact_path.exists());
    let warnings = documentation.warnings().diagnostics();
    assert_eq!(warnings.len(), 1);
    let span = warnings[0].span.unwrap();
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "Missing");
    assert!(
        documentation.warnings().structured().unwrap()[0]
            .message()
            .contains("Missing")
    );
    let bytes = std::fs::read(&input_path).unwrap();
    let input = scoop_doc::ConeInput::from_json(&bytes).unwrap();
    assert_eq!(input.sources.len(), 1);
    assert_eq!(input.sources[0].text, SOURCE);
    let answer = input
        .items
        .iter()
        .find(|item| item.name == "answer")
        .unwrap();
    assert_eq!(answer.aliases, ["result"]);
    assert_eq!(
        answer.comment.as_ref().unwrap().body.text,
        "文档 [Missing]."
    );
    assert_eq!(input.warnings.len(), 1);

    let mut combined = build_manifest_request(
        workspace.path(),
        &target,
        &root,
        &artifact_path,
        Vec::new(),
        Vec::new(),
    );
    combined.output = SingleConeOutput::ArtifactWithDocumentation {
        artifact: SlibOutputDestination::new(&artifact_path).unwrap(),
        documentation: DocumentationOutputDestination::new(&input_path).unwrap(),
    };
    let combined = combined.build_and_publish().unwrap();
    assert!(artifact_path.is_file());
    assert_eq!(std::fs::read(&input_path).unwrap(), bytes);
    assert_eq!(
        combined
            .documentation()
            .unwrap()
            .warnings()
            .diagnostics()
            .len(),
        1
    );

    let ordinary = build_manifest(
        workspace.path(),
        &target,
        &root,
        &workspace.path().join("ordinary.slib"),
    );
    assert!(ordinary.documentation().is_none());
    assert_artifacts_equal(&combined, &ordinary);
    let artifact_bytes = std::fs::read(&artifact_path).unwrap();
    for text in [SOURCE, "文档 [Missing]."] {
        assert!(
            !artifact_bytes
                .windows(text.len())
                .any(|window| window == text.as_bytes()),
            "source and documentation text must stay outside the slib"
        );
    }

    std::fs::write(
        root.join("src/main.scoop"),
        "package test\n/// Valid prose.\npublic fun answer(): Long = missingBodyValue\n",
    )
    .unwrap();
    let error = make_request().compile(false).unwrap_err();
    let diagnostics = error.structured_diagnostics().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message().contains("missingBodyValue"))
    );
    assert_eq!(std::fs::read(&input_path).unwrap(), bytes);
}

#[test]
fn documentation_core_requests_select_all_targets_without_machine_toolchains() {
    let workspace = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(workspace.path());
    let source = workspace.path().join("src/scoop.core");
    for triple in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
    ] {
        let output = workspace.path().join(format!("{triple}.json"));
        let path = |path: &Path| scoop_protocol::HostPathCarrier::from_path(path).unwrap();
        let wire = scoop_protocol::ScoopcBuildRequestV1::new(
            scoop_protocol::CurrentConeRequestV1::ExpandedCone {
                config: Box::new(
                    scoop_protocol::package::ExpandedConeInputV1::from_manifest(
                        &scoop_toolchain::load_cone_manifest(&ManifestRootLocator::cone_directory(
                            &source,
                        ))
                        .unwrap(),
                    )
                    .unwrap(),
                ),
            },
            Vec::new(),
            Vec::new(),
            scoop_protocol::TrustedCoreRequestV1::Bootstrap,
            scoop_protocol::TargetSelectionRequestV1::new(triple.into()).unwrap(),
            scoop_protocol::ScoopcOutputRequestV1::Documentation(path(&output)),
            scoop_protocol::DiagnosticOutputPolicyV1::Structured,
            scoop_protocol::StageDumpPolicyV1::None,
        )
        .unwrap();
        let request = crate::normalize_protocol_build_request(&wire).unwrap();
        assert!(matches!(
            request.target,
            crate::request::target::SelectedTarget::Frontend(_)
        ));
        let result = request.compile(false).unwrap();
        let SingleConeCompilation::Documented(documented) = result else {
            panic!("core documentation must not require machine production");
        };
        assert!(documented.documentation().warnings().is_empty());
        let input = scoop_doc::ConeInput::from_json(&std::fs::read(&output).unwrap()).unwrap();
        assert!(input.items.iter().any(|item| item.name == "Int"));
    }
}
