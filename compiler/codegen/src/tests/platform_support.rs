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
