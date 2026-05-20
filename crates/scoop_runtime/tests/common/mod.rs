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

fn gc_is_baseline_like() -> bool {
    cfg!(feature = "gc-baseline")
        || !cfg!(any(
            feature = "gc-immix",
            feature = "gc-hosted",
            feature = "gc-minimal"
        ))
}

pub(crate) fn gc_supports_stw() -> bool {
    gc_is_baseline_like() || cfg!(feature = "gc-immix")
}

pub(crate) fn gc_supports_multi_thread_roots_enum() -> bool {
    gc_is_baseline_like() || cfg!(feature = "gc-immix")
}

pub(crate) fn gc_capabilities_debug() -> String {
    format!(
        "stw={}, multi_thread_roots_enum={}",
        gc_supports_stw(),
        gc_supports_multi_thread_roots_enum()
    )
}
