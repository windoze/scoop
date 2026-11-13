//! Host sources used by executable C runtime tests.

#[cfg(unix)]
pub(crate) fn native_os_source() -> &'static str {
    match std::env::consts::OS {
        "linux" => "runtime/src/platform/os/linux.c",
        "macos" => "runtime/src/platform/os/darwin.c",
        host => panic!("no runtime test OS component for {host}"),
    }
}

pub(crate) fn native_platform_bundle_source() -> &'static str {
    native_runtime_profile()
        .runtime_sources(false)
        .into_iter()
        .find(|source| *source == "runtime/src/platform/bundle.c")
        .expect("the native runtime has a platform bundle")
}

pub(crate) fn native_runtime_c_flags() -> Vec<&'static str> {
    native_runtime_profile().runtime_c_flags()
}

#[cfg(unix)]
pub(crate) fn native_float_environment_source() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "runtime/src/platform/arch/x86_64_float_environment.c",
        "aarch64" => "runtime/src/platform/arch/aarch64_float_environment.c",
        host => panic!("no runtime floating environment component for {host}"),
    }
}

#[cfg(unix)]
pub(crate) fn native_trace_module_sources() -> &'static [&'static str] {
    match std::env::consts::OS {
        "linux" => &[
            "runtime/src/stack_trace/elf.c",
            "runtime/src/stack_trace/linux.c",
        ],
        "macos" => &["runtime/src/stack_trace/macho.c"],
        host => panic!("no stack-trace module source for {host}"),
    }
}

/// Link the actual symbolizer into focused native runtime tests.
#[cfg(unix)]
pub(crate) fn native_trace_symbols(
    command: &mut std::process::Command,
    workspace: &std::path::Path,
) {
    let profile = native_runtime_profile();
    command
        .arg("-I")
        .arg(workspace.join("sysroot/vendor/libbacktrace"));
    command.arg(workspace.join("runtime/src/stack_trace/symbolize.c"));
    for source in profile.runtime_sources(false) {
        if source.starts_with("vendor/libbacktrace/") {
            command.arg(workspace.join("sysroot").join(source));
        }
    }
}

fn native_runtime_profile() -> scoop_toolchain::ValidatedRuntimeBuildProfile {
    let host = scoop_target_data::host(std::env::consts::OS, std::env::consts::ARCH, "")
        .expect("supported runtime test host");
    let target = scoop_target_data::named_target(host.default_target)
        .unwrap()
        .selection
        .target();
    scoop_toolchain::ValidatedRuntimeBuildProfile::for_target(target).unwrap()
}
