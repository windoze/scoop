use super::*;

#[test]
fn core_child_failure_never_publishes_a_cache_entry() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let prepared = prepare(&root, workspace).unwrap();
    let key = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let store = CompileCacheStoreV1::new(&ArtifactCacheRoot::new(workspace.join("cache")).unwrap());
    assert!(matches!(
        prepared.execute_ordinary_source(
            ConeIdentity::CORE,
            &[],
            &FailureRunner,
            RequestCorrelationId::from_array([31; 16]),
        ),
        Err(OrdinarySourceExecutionError::ChildFailure(diagnostics))
            if diagnostics.len() == 1
    ));
    assert!(!store.entry_path(key).exists());
}

#[test]
fn scheduler_stops_before_dependent_after_core_failure() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let prepared = prepare(&root, workspace).unwrap();
    let core_root = prepared
        .source_input_path(ConeIdentity::CORE)
        .unwrap()
        .to_path_buf();
    let runner = RecordingFailureRunner::default();

    let error = prepared.execute_with_runner(&runner).unwrap_err();
    assert!(matches!(
        error.cause(),
        BuildGraphExecutionError::Ordinary(ConeIdentity::CORE, source)
            if matches!(source.as_ref(), OrdinarySourceExecutionError::ChildFailure(_))
    ));
    assert_eq!(
        runner.current.into_inner().unwrap(),
        vec![CurrentConeRequestV1::ManifestRoot {
            root: scoop_protocol::HostPathCarrier::from_path(&core_root).unwrap()
        }]
    );
}

#[test]
fn core_source_changes_update_the_compile_key() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));

    let prepared = prepare(&root, workspace).unwrap();
    let initial = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    assert_eq!(
        prepared.source_input_path(ConeIdentity::CORE).unwrap(),
        sysroot.join("src/scoop.core").canonicalize().unwrap()
    );
    std::fs::write(
        sysroot.join("src/scoop.core/src/core.scoop"),
        "class Any\nclass Unit\n",
    )
    .unwrap();

    let next = prepare(&root, workspace).unwrap();
    assert_ne!(
        next.compile_cache_key(ConeIdentity::CORE, &[]).unwrap(),
        initial
    );
}

#[test]
fn core_child_plan_uses_the_actual_manifest_root() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let mut prepared = prepare(&root, workspace).unwrap();
    let request_id = RequestCorrelationId::from_array([4; 16]);

    let plan = prepared
        .child_invocation_plan(ConeIdentity::CORE, request_id, &[])
        .unwrap();

    assert_eq!(plan.identity(), ConeIdentity::CORE);
    assert_eq!(
        plan.output_path(),
        prepared.planned_output_path(ConeIdentity::CORE).unwrap()
    );
    assert_eq!(plan.request().request_id(), request_id);
    assert!(matches!(
        plan.request().build().current(),
        CurrentConeRequestV1::ManifestRoot { root }
            if root.to_path_buf().unwrap() == prepared.source_input_path(ConeIdentity::CORE).unwrap()
    ));
    assert!(plan.request().build().direct_slibs().is_empty());
    assert!(plan.request().build().support_slibs().is_empty());
    assert!(matches!(
        plan.request().build().trusted_core(),
        TrustedCoreRequestV1::Bootstrap
    ));
    assert_eq!(
        plan.request().build().diagnostics(),
        DiagnosticOutputPolicyV1::Structured
    );
    assert_eq!(plan.request().build().emit(), &StageDumpPolicyV1::None);
    assert_eq!(
        plan.request().build().target().canonical_triple(),
        scoop_toolchain::host_target_triple().unwrap()
    );
    prepared.source_inputs.remove(&ConeIdentity::CORE).unwrap();
    assert!(matches!(
        prepared.child_invocation_plan(ConeIdentity::CORE, request_id, &[]),
        Err(crate::ChildRequestPlanError::MissingSourceProjection(identity)) if identity == ConeIdentity::CORE
    ));
}
