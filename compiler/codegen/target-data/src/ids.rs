use scoop_identity::TargetProfileId;

const fn id(name: &str) -> TargetProfileId {
    match TargetProfileId::new(name, 1) {
        Ok(id) => id,
        Err(_) => panic!("invalid registered target identity"),
    }
}

pub const DARWIN_AARCH64_ID: TargetProfileId = id("darwin-aarch64");
pub const LINUX_X86_64_GNU_ID: TargetProfileId = id("linux-x86-64-gnu");
pub const LINUX_X86_64_MUSL_ID: TargetProfileId = id("linux-x86-64-musl");
pub const WINDOWS_X86_64_MSVC_UCRT_ID: TargetProfileId = id("windows-x86-64-msvc-ucrt");
pub const WINDOWS_X86_64_MINGW_MSVCRT_ID: TargetProfileId = id("windows-x86-64-llvm-mingw-msvcrt");
pub const WINDOWS_X86_64_MINGW_UCRT_ID: TargetProfileId = id("windows-x86-64-llvm-mingw-ucrt");
