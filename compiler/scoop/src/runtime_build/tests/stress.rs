use super::*;

#[test]
fn diagnostic_options_select_distinct_reusable_runtime_objects() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let build = |enable_gc_stress, enable_startup_stats| {
        build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &runtime_root(),
            vendor_root: &vendor_root(),
            cache_root: cache.path(),
            optimization: RuntimeOptimization::Optimized,
            enable_gc_stress,
            enable_startup_stats,
            unwind_prefix: None,
        })
        .unwrap()
    };
    let mut indices = std::collections::BTreeSet::new();
    let mut fingerprints = Vec::new();
    for (stress, startup) in [(false, false), (true, false), (false, true), (true, true)] {
        let first = build(stress, startup);
        assert!(!first.cache_hit());
        assert!(indices.insert(first.index().to_path_buf()));
        let fingerprint = first.objects().fingerprint();
        assert!(!fingerprints.contains(&fingerprint));
        fingerprints.push(fingerprint);
        let cached = build(stress, startup);
        assert!(cached.cache_hit());
        assert_eq!(cached.index(), first.index());
        assert_eq!(cached.objects().configuration().enable_gc_stress, stress);
        assert_eq!(
            cached.objects().configuration().enable_startup_stats,
            startup
        );
        cached.objects().require_gc_stress(stress).unwrap();
        assert!(cached.objects().require_gc_stress(!stress).is_err());
        cached.objects().require_startup_stats(startup).unwrap();
        assert!(cached.objects().require_startup_stats(!startup).is_err());
        let objects: Vec<_> = cached
            .objects()
            .objects()
            .iter()
            .map(|object| std::fs::read(object.path()).unwrap())
            .collect();
        for (marker, expected) in [
            (b"SCOOP_GC_STRESS_".as_slice(), stress),
            (b"{\"scoop_startup\":1".as_slice(), startup),
            (b"cannot measure startup time".as_slice(), startup),
            (b"scoop_image_scan_program_count".as_slice(), startup),
        ] {
            assert_eq!(
                objects
                    .iter()
                    .any(|bytes| bytes.windows(marker.len()).any(|window| window == marker)),
                expected,
                "marker {marker:?}"
            );
        }
    }
}
