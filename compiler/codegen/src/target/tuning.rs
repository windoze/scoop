//! Measured target choices for existing optimizations, independent of LLVM names.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TargetTuning {
    pub reuse_interface_values: bool,
    pub direct_array_regions: bool,
    pub allocation_initialization_limit: u64,
    pub optimize_tlab_range: bool,
    pub reuse_thread_addresses: bool,
    pub retain_card_stack_reloads: bool,
}

impl TargetTuning {
    pub(super) const fn for_profile(profile: scoop_target_data::CodegenTuning) -> Self {
        match profile {
            scoop_target_data::CodegenTuning::DarwinAarch64 => Self {
                // Snapshot forwarding regressed reference-copy kernels.
                reuse_interface_values: false,
                direct_array_regions: true,
                allocation_initialization_limit: 128,
                optimize_tlab_range: true,
                reuse_thread_addresses: true,
                retain_card_stack_reloads: true,
            },
            scoop_target_data::CodegenTuning::X86_64 => Self {
                reuse_interface_values: true,
                // Direct queries, larger initialization regions, range folding
                // and the extra TLS-cache register regressed measured kernels.
                direct_array_regions: false,
                allocation_initialization_limit: 64,
                optimize_tlab_range: false,
                reuse_thread_addresses: false,
                retain_card_stack_reloads: false,
            },
        }
    }
}
