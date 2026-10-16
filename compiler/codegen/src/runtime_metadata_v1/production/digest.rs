//! Physical digest slots collected across the emitted object set.

use super::ProvisionalStrongDigestPatchLocationV1;
use crate::CodegenError;
use scoop_lir::{DigestPatchIntentId, ObjectDefinitionPlanId};

#[derive(Clone, Copy)]
pub(super) struct PatchParts {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    byte_offset: u64,
}

impl PatchParts {
    pub(super) fn from_entry(patch: super::super::RootEntryPatchSiteV1<'_>) -> Self {
        Self {
            intent: patch.intent(),
            definition: patch.definition(),
            byte_offset: patch.byte_offset(),
        }
    }
}

pub(super) trait PatchSiteParts<'ctx> {
    fn into_parts(self) -> PatchParts;
}

macro_rules! impl_patch_site_parts {
    ($($ty:ident),+ $(,)?) => {$(
        impl<'ctx> PatchSiteParts<'ctx> for super::super::$ty<'ctx> {
            fn into_parts(self) -> PatchParts {
                PatchParts {
                    intent: self.intent(),
                    definition: self.definition(),
                    byte_offset: self.byte_offset(),
                }
            }
        }
    )+};
}

impl_patch_site_parts!(
    SafepointRegistrationPatchSiteV1,
    CallableRegistrationPatchSiteV1,
    TypeRegistrationPatchSiteV1,
    StaticStorageRegistrationPatchSiteV1,
    InitializationRegistrationPatchSiteV1,
    RuntimeImagePatchSiteV1,
);

pub(super) fn record_patch<D, C, I>(
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    patches: &mut Vec<ProvisionalStrongDigestPatchLocationV1>,
    patch: PatchParts,
) -> Result<(), CodegenError> {
    let definition = production
        .canonical_definitions()
        .plan(patch.definition)
        .ok_or_else(|| {
            CodegenError(format!(
                "emitted digest patch {:?} references unknown definition {:?}",
                patch.intent, patch.definition
            ))
        })?;
    patches.push(ProvisionalStrongDigestPatchLocationV1 {
        intent: patch.intent,
        definition: patch.definition,
        atom: definition.primary_atom(),
        owner: definition.primary_symbol(),
        offset_within_owner: patch.byte_offset,
        width_bytes: 32,
    });
    Ok(())
}
