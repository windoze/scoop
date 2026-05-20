pub(crate) fn gc_backend_name() -> &'static str {
    if cfg!(feature = "gc-immix") {
        "Immix"
    } else if cfg!(feature = "gc-hosted") {
        "Hosted"
    } else if cfg!(feature = "gc-minimal") {
        "Minimal"
    } else {
        "Baseline"
    }
}

pub(crate) fn gc_supports_stw() -> bool {
    cfg!(any(feature = "gc-baseline", feature = "gc-immix"))
}

pub(crate) fn gc_supports_multi_thread_roots_enum() -> bool {
    cfg!(any(feature = "gc-baseline", feature = "gc-immix"))
}

pub(crate) fn gc_capabilities_debug() -> String {
    format!(
        "stw={}, multi_thread_roots_enum={}",
        gc_supports_stw(),
        gc_supports_multi_thread_roots_enum()
    )
}
