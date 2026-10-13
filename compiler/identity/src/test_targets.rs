use crate::TargetProfileId;

const fn id(name: &str) -> TargetProfileId {
    match TargetProfileId::new(name, 1) {
        Ok(id) => id,
        Err(_) => panic!("invalid test target identity"),
    }
}

pub const DARWIN_AARCH64_ID: TargetProfileId = id("darwin-aarch64");
pub const LINUX_X86_64_GNU_ID: TargetProfileId = id("linux-x86-64-gnu");
pub const LINUX_X86_64_MUSL_ID: TargetProfileId = id("linux-x86-64-musl");
