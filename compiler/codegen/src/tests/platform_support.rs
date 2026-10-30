//! Host sources used by executable C runtime tests.

#[cfg(unix)]
pub(crate) fn native_os_source() -> &'static str {
    match std::env::consts::OS {
        "linux" => "runtime/src/platform/os/linux.c",
        "macos" => "runtime/src/platform/os/darwin.c",
        host => panic!("no runtime test OS component for {host}"),
    }
}

pub(crate) fn native_profile_source() -> &'static str {
    native_runtime_profile()
        .runtime_sources()
        .into_iter()
        .find(|source| source.starts_with("runtime/src/platform/profiles/"))
        .expect("the native runtime has a platform profile")
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
pub(crate) fn native_trace_module_source() -> &'static str {
    match std::env::consts::OS {
        "linux" => "runtime/src/stack_trace/elf.c",
        "macos" => "runtime/src/stack_trace/macho.c",
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
    for source in profile.runtime_sources() {
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
