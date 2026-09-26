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
    let mut prepared = prepare(&root, workspace).unwrap();
    let key = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let store = CompileCacheStoreV1::new(&ArtifactCacheRoot::new(workspace.join("cache")).unwrap());
    assert!(matches!(
        prepared.execute_ordinary_source(
            ConeIdentity::CORE,
            &[],
            &mut FailureRunner,
            RequestCorrelationId::from_array([31; 16]),
        ),
        Err(OrdinarySourceExecutionError::ChildFailure(diagnostics))
            if diagnostics.len() == 1
    ));
    assert!(!store.entry_path(key).exists());
}

#[test]
fn serial_scheduler_stops_before_dependent_after_core_failure() {
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
    let mut runner = RecordingFailureRunner::default();

    assert!(matches!(
        prepared.execute_with_runner(&mut runner),
        Err(BuildGraphExecutionError::Ordinary(ConeIdentity::CORE, source))
            if matches!(source.as_ref(), OrdinarySourceExecutionError::ChildFailure(_))
    ));
    assert_eq!(
        runner.current,
        vec![CurrentConeRequestV1::ManifestRoot {
            root: scoop_protocol::HostPathCarrier::from_path(&core_root).unwrap()
        }]
    );
}

#[test]
fn core_incompatible_profile_never_publishes_a_cache_entry() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let mut prepared = prepare(&root, workspace).unwrap();
    let key = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let store = CompileCacheStoreV1::new(&ArtifactCacheRoot::new(workspace.join("cache")).unwrap());
    assert!(matches!(
        prepared.execute_ordinary_source(
            ConeIdentity::CORE,
            &[],
            &mut IncompatibleProfileSuccessRunner,
            RequestCorrelationId::from_array([33; 16]),
        ),
        Err(OrdinarySourceExecutionError::Completion(
            CompiledCompletionError::Plan(source)
        )) if matches!(source.as_ref(), crate::ArtifactClosureValidationError::ProfileMismatch { .. })
    ));
    assert!(!store.entry_path(key).exists());
}

#[test]
fn core_uses_the_common_immutable_snapshot_and_cache_key() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));

    let prepared = prepare(&root, workspace).unwrap();
    let initial = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let private_root = prepared.source_input_path(ConeIdentity::CORE).unwrap();
    std::fs::write(
        sysroot.join("lib/scoop.core/src/core.scoop"),
        "class Any\nclass Unit\n",
    )
    .unwrap();

    assert_eq!(
        std::fs::read(private_root.join("src/core.scoop")).unwrap(),
        b"class Any\n"
    );
    assert_eq!(
        prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap(),
        initial
    );
    let next = prepare(&root, workspace).unwrap();
    assert_ne!(
        next.compile_cache_key(ConeIdentity::CORE, &[]).unwrap(),
        initial
    );
}

#[test]
fn core_child_plan_uses_the_common_manifest_snapshot_request() {
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
    assert_eq!(plan.request().build().emit(), StageDumpPolicyV1::None);
    assert_eq!(
        plan.request().build().target().canonical_triple(),
        "aarch64-apple-darwin"
    );
    prepared.source_inputs.remove(&ConeIdentity::CORE).unwrap();
    assert!(matches!(
        prepared.child_invocation_plan(ConeIdentity::CORE, request_id, &[]),
        Err(crate::ChildRequestPlanError::MissingSourceProjection(identity)) if identity == ConeIdentity::CORE
    ));
}
