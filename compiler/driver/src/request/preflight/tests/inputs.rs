use super::*;

#[test]
fn manifest_dependency_is_loaded_without_discovering_current_sources() {
    let directory = tempfile::tempdir().unwrap();
    let manifest_path = directory.path().join("Cone.toml");
    let text = "schema = 1\n[cone]\ngroup = \"test\"\nname = \"current\"\nversion = \"0.0.0\"\nkind = \"library\"\n[dependencies]\n\"dev.example:dep\" = \"1.2.3\"\n";
    std::fs::write(&manifest_path, text).unwrap();
    let current = load_current_input(CurrentConeInput::Manifest {
        root: ManifestRootLocator::exact_manifest_file(manifest_path),
    })
    .unwrap();
    let LoadedCurrentConeInput::Manifest { manifest } = current else {
        panic!("test constructs a manifest input")
    };
    let (key, coordinate) = manifest
        .parsed()
        .semantic()
        .dependency_iter()
        .next()
        .unwrap();
    let declaration = manifest
        .parsed()
        .diagnostic_spans()
        .dependency(key)
        .unwrap()
        .declaration();
    assert_eq!(
        coordinate,
        &ConeCoordinate::new("dev.example", "dep", "1.2.3").unwrap()
    );
    assert_eq!(&text[declaration.range()], "\"1.2.3\"");
    assert!(!directory.path().join("src").exists());
}

#[test]
fn explicit_artifact_is_read_during_preflight() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.slib");
    let dependencies = ExplicitDependencyInputs::new(
        vec![HostArtifactLocator::new(&missing).unwrap()],
        Vec::new(),
    )
    .unwrap();

    assert!(matches!(
        LoadedExplicitDependencyInputs::load(
            dependencies.direct(),
            dependencies.support(),
            DecodeLimits::default(),
        ),
        Err(ExplicitDependencyLoadError::Io {
            input,
            operation: ExplicitDependencyLoadOperation::Open,
            ..
        }) if input.role() == ExplicitDependencyRole::Direct
            && input.index() == 0
            && input.path() == missing
    ));
}

#[test]
fn empty_core_dependencies_run_the_shared_closure() {
    let Ok(target) = scoop_toolchain::ResolvedTargetProfile::resolve_host() else {
        return;
    };
    let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap();
    let loaded = LoadedExplicitDependencyInputs::load(
        dependencies.direct(),
        dependencies.support(),
        DecodeLimits::default(),
    )
    .unwrap();

    let validated = loaded
        .validate_inner(None, ConeIdentity::CORE, &target)
        .unwrap();
    assert!(validated.is_empty());
    assert!(validated.dependency_first().is_empty());
    assert!(validated.direct_dependencies().is_empty());
    assert_eq!(validated.semantic().current(), ConeIdentity::CORE);
    let world = validated.semantic().imported_semantic_world().unwrap();
    assert_eq!(world.current(), ConeIdentity::CORE);
    assert_eq!(world.provider_count(), 0);

    assert!(matches!(
        loaded.validate_inner(None, ConeIdentity::SINGLE_FILE, &target),
        Err(error) if matches!(error.as_ref(), ExplicitDependencyValidationError::ManifestDirectSet { declared, actual }
            if declared == &[ConeCoordinate::reserved_core()] && actual.is_empty()),
    ));
}

#[test]
fn core_dependency_inputs_use_the_shared_artifact_summary_validation() {
    let Ok(target) = scoop_toolchain::ResolvedTargetProfile::resolve_host() else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("malformed.slib");
    std::fs::write(&path, b"malformed library").unwrap();
    let loaded = LoadedExplicitDependencyInputs::load(
        &[HostArtifactLocator::new(&path).unwrap()],
        &[],
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        loaded.validate_inner(None, ConeIdentity::CORE, &target),
        Err(error) if matches!(error.as_ref(), ExplicitDependencyValidationError::Summary { input, .. }
            if input.path() == path),
    ));
}

#[test]
fn manifest_sources_parse_in_canonical_identity_order() {
    let directory = manifest_cone();
    std::fs::write(directory.path().join("src/z.scoop"), "package z\n").unwrap();
    std::fs::write(directory.path().join("src/a.scoop"), "package a\n").unwrap();
    let current = load_current_input(CurrentConeInput::Manifest {
        root: ManifestRootLocator::cone_directory(directory.path()),
    })
    .unwrap();

    let LoadedCurrentConeInput::Manifest { manifest } = &current else {
        panic!("test constructs a manifest input")
    };
    let parsed = parse_manifest_current(manifest).unwrap();

    let paths = parsed
        .sources()
        .sources()
        .iter()
        .map(|source| source.identity().logical_path().as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["src/a.scoop", "src/z.scoop"]);
    assert_eq!(parsed.source_texts().entries().len(), 2);
    assert_eq!(parsed.diagnostic_context().entries().len(), 2);
}

#[test]
fn single_file_parse_uses_the_reserved_source_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("arbitrary-name.scoop");
    std::fs::write(&path, "fun main() {}\n").unwrap();
    let current = load_current_input(CurrentConeInput::SingleFile {
        source: SingleFileLocator::from_path(&path).unwrap(),
    })
    .unwrap();

    let LoadedCurrentConeInput::SingleFile { source } = &current else {
        panic!("test constructs a single-file input")
    };
    let parsed = parse_single_file_current(source).unwrap();

    assert_eq!(parsed.cone(), scoop_identity::ConeIdentity::SINGLE_FILE);
    assert_eq!(
        parsed.sources().sources().first().identity(),
        &SourceIdentity::single_file()
    );
    assert_eq!(
        parsed
            .diagnostic_context()
            .entries()
            .first()
            .display_locator(),
        path
    );
    let warnings = CurrentConeDiagnosticSet::try_new(
        vec![scoop_ast::Diagnostic::warning_at(
            scoop_ast::Span::new(4, 8),
            "entry warning",
        )],
        &parsed,
    )
    .unwrap();
    assert_eq!(warnings.diagnostics().len(), 1);
    assert_eq!(
        warnings.render_human(),
        format!("{}:1:5: warning: entry warning", path.display())
    );
}

#[test]
fn parser_failure_keeps_semantic_source_identity() {
    let directory = manifest_cone();
    std::fs::write(directory.path().join("src/bad.scoop"), "package .bad\n").unwrap();
    let current = load_current_input(CurrentConeInput::Manifest {
        root: ManifestRootLocator::cone_directory(directory.path()),
    })
    .unwrap();

    let LoadedCurrentConeInput::Manifest { manifest } = &current else {
        panic!("test constructs a manifest input")
    };
    let error = parse_manifest_current(manifest).unwrap_err();

    assert!(matches!(
        error,
        CurrentConeSourceStageError::Parser(source)
            if matches!(source.as_ref(), ParseCurrentConeError::Diagnostics(diagnostics)
                if diagnostics.len() == 1
                    && diagnostics[0].identity().logical_path().as_str() == "src/bad.scoop")
    ));
}

fn manifest_cone() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(
            directory.path().join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"test\"\nname = \"current\"\nversion = \"0.0.0\"\nkind = \"library\"\n",
        )
        .unwrap();
    directory
}
