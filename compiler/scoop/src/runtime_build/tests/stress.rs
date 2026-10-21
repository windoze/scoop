use super::*;

#[test]
fn stress_support_selects_distinct_reusable_runtime_objects() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let build = |enable_gc_stress| {
        build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &runtime_root(),
            vendor_root: &vendor_root(),
            cache_root: cache.path(),
            optimization: RuntimeOptimization::Optimized,
            enable_gc_stress,
            unwind_prefix: None,
        })
        .unwrap()
    };
    let ordinary = build(false);
    let stress = build(true);
    assert!(!ordinary.cache_hit());
    assert!(!stress.cache_hit());
    assert_ne!(ordinary.index(), stress.index());
    assert_ne!(
        ordinary.objects().fingerprint(),
        stress.objects().fingerprint()
    );
    for (enabled, first) in [(false, ordinary), (true, stress)] {
        let cached = build(enabled);
        assert!(cached.cache_hit());
        assert_eq!(cached.index(), first.index());
        assert_eq!(cached.objects().configuration().enable_gc_stress, enabled);
        cached.objects().require_gc_stress(enabled).unwrap();
        assert!(cached.objects().require_gc_stress(!enabled).is_err());
        let has_stress_variables = cached.objects().objects().iter().any(|object| {
            let bytes = std::fs::read(object.path()).unwrap();
            bytes
                .windows(b"SCOOP_GC_STRESS_".len())
                .any(|window| window == b"SCOOP_GC_STRESS_")
        });
        assert_eq!(has_stress_variables, enabled);
    }
}
