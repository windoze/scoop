//! Host sources used by executable C runtime tests.

pub(crate) fn native_os_source() -> &'static str {
    match std::env::consts::OS {
        "linux" => "runtime/src/platform/os/linux.c",
        "macos" => "runtime/src/platform/os/darwin.c",
        host => panic!("no runtime test OS component for {host}"),
    }
}

pub(crate) fn native_profile_source() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "runtime/src/platform/profiles/linux_x86_64.c",
        ("macos", "aarch64") => "runtime/src/platform/profiles/darwin_aarch64.c",
        host => panic!("no runtime test profile for {host:?}"),
    }
}

pub(crate) fn native_float_environment_source() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "runtime/src/platform/arch/x86_64_float_environment.c",
        "aarch64" => "runtime/src/platform/arch/aarch64_float_environment.c",
        host => panic!("no runtime floating environment component for {host}"),
    }
}

pub(crate) fn native_trace_module_source() -> &'static str {
    match std::env::consts::OS {
        "linux" => "runtime/src/stack_trace/elf.c",
        "macos" => "runtime/src/stack_trace/macho.c",
        host => panic!("no stack-trace module source for {host}"),
    }
}

/// Link the actual symbolizer into focused native runtime tests.
pub(crate) fn native_trace_symbols(
    command: &mut std::process::Command,
    workspace: &std::path::Path,
) {
    let target = match std::env::consts::OS {
        "linux" => scoop_target_data::LINUX_X86_64_GNU,
        "macos" => scoop_target_data::DARWIN_AARCH64,
        host => panic!("no trace symbolizer for {host}"),
    };
    let profile = scoop_toolchain::ValidatedRuntimeBuildProfile::for_target(target).unwrap();
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
