//! Closed runtime source and C-build projection; no linker or LLVM discovery.
use scoop_lir::{LirTargetProfile, TargetProfileId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedRuntimeBuildProfile {
    target: LirTargetProfile,
}

impl ValidatedRuntimeBuildProfile {
    pub const fn for_target(target: LirTargetProfile) -> Self {
        Self { target }
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.target.id().canonical_triple()
    }

    pub fn runtime_sources(self) -> Vec<&'static str> {
        let platform = match self.target.id() {
            TargetProfileId::DarwinAarch64 => DARWIN_AARCH64,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => LINUX_AMD64,
        };
        COMMON.iter().chain(platform).copied().collect()
    }

    pub const fn runtime_c_flags(self) -> &'static [&'static str] {
        match self.target.id() {
            TargetProfileId::DarwinAarch64 => &[
                "-pthread",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
            ],
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => &[
                "-pthread",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
                "-mno-red-zone",
                "-fvisibility=hidden",
                "-DMBEDTLS_CONFIG_FILE=\"scoop_sha256_config.h\"",
            ],
        }
    }

    pub const fn include_directories(self) -> &'static [&'static str] {
        match self.target.id() {
            TargetProfileId::DarwinAarch64 => &["include", "src", "third_party/ryu"],
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => &[
                "include",
                "src",
                "third_party/ryu",
                "third_party/mbedtls",
                "third_party/mbedtls/include",
            ],
        }
    }
}

const COMMON: &[&str] = &[
    "runtime/src/rt.c",
    "runtime/src/characters.c",
    "runtime/src/floating.c",
    "runtime/src/floating_parse.c",
    "runtime/third_party/ryu/ryu/f2s.c",
    "runtime/third_party/ryu/ryu/d2s.c",
    "runtime/src/strings.c",
    "runtime/src/string_parts.c",
    "runtime/src/utf8.c",
    "runtime/src/startup.c",
    "runtime/src/startup/failure.c",
    "runtime/src/startup/gateway.c",
    "runtime/src/boxing.c",
    "runtime/src/arrays.c",
    "runtime/src/task_context.c",
    "runtime/src/value_shape.c",
    "runtime/src/value_scan.c",
    "runtime/src/eh.c",
    "runtime/src/eh_personality.c",
    "runtime/src/eh/lsda.c",
    "runtime/src/initialization.c",
    "runtime/src/image/ranges.c",
    "runtime/src/image/checks.c",
    "runtime/src/image/dependencies.c",
    "runtime/src/image/records.c",
    "runtime/src/image/registry.c",
    "runtime/src/image/lookup.c",
    "runtime/src/image/active.c",
    "runtime/src/image/scan_ranges.c",
    "runtime/src/image/types.c",
    "runtime/src/image/context_keys.c",
    "runtime/src/image/type_relations.c",
    "runtime/src/image/storage.c",
    "runtime/src/image/immortals.c",
    "runtime/src/image/static_values.c",
    "runtime/src/image/units.c",
    "runtime/src/image/allocation_ranges.c",
    "runtime/src/image/stackmaps.c",
    "runtime/src/gc.c",
    "runtime/src/gc/allocation.c",
    "runtime/src/gc/collector.c",
    "runtime/src/gc/evacuation.c",
    "runtime/src/gc/reclamation.c",
    "runtime/src/gc/heap.c",
    "runtime/src/gc/heap_objects.c",
    "runtime/src/gc/handles.c",
    "runtime/src/gc/root_frames.c",
    "runtime/src/gc/roots.c",
    "runtime/src/gc/stackmap.c",
    "runtime/src/gc/stackmap/parser.c",
    "runtime/src/gc/stackmap/records.c",
    "runtime/src/gc/stackmap/fingerprint.c",
    "runtime/src/gc/stack_roots.c",
    "runtime/src/thread.c",
    "runtime/src/thread/collection.c",
    "runtime/src/thread/debug.c",
    "runtime/src/thread/roots.c",
    "runtime/src/thread/transitions.c",
    "runtime/src/callback.c",
    "runtime/src/platform/common.c",
];

const DARWIN_AARCH64: &[&str] = &[
    "runtime/src/platform/profiles/darwin_aarch64.c",
    "runtime/src/platform/image/macho.c",
    "runtime/src/platform/image/darwin_sha256.c",
    "runtime/src/platform/arch/aarch64.c",
    "runtime/src/platform/arch/aarch64_anchor.S",
    "runtime/src/platform/arch/aarch64_strings.S",
    "runtime/src/platform/arch/aarch64_floating.S",
    "runtime/src/platform/os/darwin.c",
];

const LINUX_AMD64: &[&str] = &[
    "runtime/src/platform/profiles/linux_x86_64.c",
    "runtime/src/platform/image/elf.c",
    "runtime/src/platform/image/portable_sha256.c",
    "runtime/src/platform/os/linux.c",
    "runtime/src/platform/arch/x86_64.c",
    "runtime/src/platform/arch/x86_64_anchor.S",
    "runtime/src/platform/arch/x86_64_strings.c",
    "runtime/src/platform/arch/x86_64_floating.c",
    "runtime/third_party/mbedtls/library/sha256.c",
    "runtime/third_party/mbedtls/library/platform_util.c",
];
