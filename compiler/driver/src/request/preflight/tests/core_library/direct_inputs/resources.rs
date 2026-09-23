use super::*;

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    workspace: &Path,
    core: &Path,
) {
    let copy = workspace.join("snapshot-core.slib");
    std::fs::copy(core, &copy).unwrap();
    let absent = workspace.join("snapshot-unused-sysroot");
    let output = workspace.join("snapshot-consumer.slib");
    let build = || request(target, root, &output, &absent, &[&copy], &[]);
    let mut limits = SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
    limits.artifact_snapshot_bytes = 1;
    let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::new(limits).unwrap());
    assert!(
        matches!(build().load_preflight_inner(DecodeLimits::default(), Some(&mut meter)),
            Err(SingleConePreflightError::ExplicitDependencyLoad(error))
                if matches!(*error, ExplicitDependencyLoadError::Resource(_))
        )
    );
    let mut limits = SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
    limits.logical_heap_bytes = 1;
    let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::new(limits).unwrap());
    assert!(
        matches!(build().load_preflight_inner(DecodeLimits::default(), Some(&mut meter)),
            Err(SingleConePreflightError::Dependencies(error))
                if matches!(*error, ExplicitDependencyValidationError::Resource(_))
        )
    );
    let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
    let loaded = build()
        .load_preflight_inner(DecodeLimits::default(), Some(&mut meter))
        .unwrap();
    let usage = meter.usage();
    std::fs::write(&copy, b"changed after the snapshot was opened").unwrap();
    assert!(
        loaded
            .dependencies
            .contains_explicit_core(target.lir_target_selection(), Some(&mut meter))
            .unwrap()
    );
    assert_eq!(meter.usage(), usage);
    loaded.validate_inner(Some(&mut meter)).unwrap();
    assert!(!absent.exists());
}
