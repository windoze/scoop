use super::*;
use crate::tests::m23_generic_body_consumption::with_provider_consumer;

#[test]
fn dependency_effects_keep_generic_substitutions_without_reanalyzing_bodies() {
    with_provider_consumer(r#"
        @NoGC public fun <T> identity(value: T): T = value
        @NoGC public fun plain(value: Int): Int = value
        @Extern(name = "m24_native") public fun native(value: Int): Int
        @NoGC @Unsafe public fun transition(value: Int): Int = native(value)
    "#, r#"
        @NoGC public fun <T> forward(value: T): T = identity(value)
        @NoGC public fun concrete(value: Int): Int = identity(plain(value))
        @NoGC @Unsafe public fun indirect(value: Int): Int = transition(value)
    "#, |output, _, _, interface, _| {
        let export = output.output().export.module();
        available(export, "forward", &["T"]);
        available(export, "concrete", &[]);
        unavailable(export, "indirect");
        let record = interface.callable_interfaces().records().iter()
            .find(|record| matches!(record.effects().release_callability(),
                hir::CallableReleaseCallabilityV1::NoTransition { requirements } if !requirements.is_empty()))
            .unwrap();
        assert_eq!(record.effects().release_callability(),
            &hir::CallableReleaseCallabilityV1::NoTransition {
                requirements: vec![hir::ReleaseValueBinderV1 { depth: 0, index: 0 }],
            });
    }).unwrap();
}
