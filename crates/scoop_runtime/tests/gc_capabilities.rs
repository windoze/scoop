use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES, GcBackend};

#[test]
fn gc_capabilities_match_selected_backend() {
    // 该测试用于保证 capability matrix 是“可被回归固化”的，而不是靠约定/注释。
    // 当新增 backend（例如 Immix/adapter）时，应在这里补齐对应的能力断言。

    if cfg!(feature = "gc-immix") {
        assert_eq!(GC_BACKEND, GcBackend::Immix);
        assert_eq!(
            GC_CAPABILITIES,
            scoop_runtime::gc_backend::GcCapabilities {
                stw: true,
                multi_thread_roots_enum: true,
                moving: true,
                precise_roots_update: true,
                shadow_stack_roots: true,
            }
        );
        return;
    }

    if cfg!(feature = "gc-minimal") {
        assert_eq!(GC_BACKEND, GcBackend::Minimal);
        assert_eq!(
            GC_CAPABILITIES,
            scoop_runtime::gc_backend::GcCapabilities {
                stw: false,
                multi_thread_roots_enum: false,
                moving: false,
                precise_roots_update: false,
                shadow_stack_roots: true,
            }
        );
        return;
    }

    assert_eq!(GC_BACKEND, GcBackend::Baseline);
    assert_eq!(
        GC_CAPABILITIES,
        scoop_runtime::gc_backend::GcCapabilities {
            stw: true,
            multi_thread_roots_enum: true,
            moving: false,
            precise_roots_update: false,
            shadow_stack_roots: true,
        }
    );
}
