//! Stable symbol names for surface-resume / continuation-driver
//! / continuation-step entry points.
//!
//! Each helper takes a published surface-resume or owner-trampoline layout
//! and produces the canonical mangled name the LLVM module will register
//! that function under. Implementations defer to `stable_naming` so the
//! mangling is consistent with the rest of the codegen pipeline.

use super::*;

pub(super) fn surface_resume_outcome_symbol_name<'ctx>(
    surface: &ContinuationSurfaceResumeLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "surface_resume__outcome",
        surface.stable_continuation_key_text(),
    )
}

pub(super) fn surface_resume_owner_outcome_symbol_name<'ctx>(
    target: &super::super::types::ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "surface_resume_owner__outcome",
        target.stable_owner_dispatch_key_text(),
    )
}

pub(super) fn surface_resume_owner_core_symbol_name<'ctx>(
    target: &super::super::types::ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "surface_resume_owner__core",
        target.stable_owner_dispatch_key_text(),
    )
}

pub(super) fn continuation_drive_outcome_symbol_name<'ctx>(
    surface: &ContinuationSurfaceResumeLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "continuation_drive__outcome",
        surface.stable_continuation_key_text(),
    )
}

pub(super) fn continuation_drive_owner_outcome_symbol_name<'ctx>(
    target: &super::super::types::ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "continuation_drive_owner__outcome",
        target.stable_owner_dispatch_key_text(),
    )
}

pub(super) fn continuation_step_symbol_name<'ctx>(
    target: &super::super::types::ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
) -> String {
    stable_naming::private_name_from_key_text(
        "continuation__step",
        target.stable_owner_dispatch_key_text(),
    )
}
