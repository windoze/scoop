//! Physical digest slots collected across the emitted object set.

use super::ProvisionalStrongDigestPatchLocationV1;
use crate::CodegenError;
use inkwell::values::GlobalValue;
use scoop_lir::{DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) struct PatchParts<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: Option<ObjectDefinitionAtomId>,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
    byte_size: u64,
}

impl<'ctx> PatchParts<'ctx> {
    pub(super) fn without_atom(patch: super::super::RootEntryPatchSiteV1<'ctx>) -> Self {
        Self {
            intent: patch.intent(),
            definition: patch.definition(),
            atom: None,
            owner: patch.owner(),
            byte_offset: patch.byte_offset(),
            byte_size: patch.byte_size(),
        }
    }
}

pub(super) trait PatchSiteParts<'ctx> {
    fn into_parts(self) -> PatchParts<'ctx>;
}

macro_rules! impl_patch_site_parts {
    ($($ty:ident),+ $(,)?) => {$(
        impl<'ctx> PatchSiteParts<'ctx> for super::super::$ty<'ctx> {
            fn into_parts(self) -> PatchParts<'ctx> {
                PatchParts {
                    intent: self.intent(),
                    definition: self.definition(),
                    atom: Some(self.atom()),
                    owner: self.owner(),
                    byte_offset: self.byte_offset(),
                    byte_size: self.byte_size(),
                }
            }
        }
    )+};
}

impl_patch_site_parts!(
    SafepointRegistrationPatchSiteV1,
    CallableRegistrationPatchSiteV1,
    TypeRegistrationPatchSiteV1,
    ImmortalObjectRegistrationPatchSiteV1,
    StaticStorageRegistrationPatchSiteV1,
    InitializationRegistrationPatchSiteV1,
    RuntimeImagePatchSiteV1,
);

pub(super) fn record_patch<D, C, I>(
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    patches: &mut Vec<ProvisionalStrongDigestPatchLocationV1>,
    patch: PatchParts<'_>,
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
    if let Some(atom) = patch.atom
        && atom != definition.primary_atom()
    {
        return Err(CodegenError(format!(
            "emitted digest patch {:?} targets atom {:?}, expected Primary atom {:?}",
            patch.intent,
            atom,
            definition.primary_atom()
        )));
    }
    let owner = definition.primary_symbol();
    if patch.owner.get_name().to_bytes() != owner.symbol().as_str().as_bytes() {
        return Err(CodegenError(format!(
            "emitted digest patch {:?} owner does not match strong definition symbol `{}`",
            patch.intent,
            owner.symbol()
        )));
    }
    let width_bytes = u8::try_from(patch.byte_size).map_err(|_| {
        CodegenError(format!(
            "emitted digest patch {:?} width {} does not fit the object sidecar",
            patch.intent, patch.byte_size
        ))
    })?;
    if width_bytes != 32 {
        return Err(CodegenError(format!(
            "emitted digest patch {:?} has width {width_bytes}, expected 32",
            patch.intent
        )));
    }
    patches.push(ProvisionalStrongDigestPatchLocationV1 {
        intent: patch.intent,
        definition: patch.definition,
        atom: definition.primary_atom(),
        owner,
        offset_within_owner: patch.byte_offset,
        width_bytes,
    });
    Ok(())
}

pub(crate) fn validate_patch_coverage<D, C, I>(
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    patches: &[ProvisionalStrongDigestPatchLocationV1],
) -> Result<(), CodegenError> {
    let expected = production
        .digest_finalization_plan()
        .nodes()
        .iter()
        .flat_map(|node| node.patch_intents())
        .map(|record| record.id())
        .collect::<BTreeSet<_>>();
    let actual = patches.iter().map(|patch| patch.intent).collect::<Vec<_>>();
    if actual.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CodegenError(
            "strong runtime metadata emitted duplicate digest patch intents".to_owned(),
        ));
    }
    let actual = actual.into_iter().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(CodegenError(format!(
            "strong runtime metadata digest patch coverage mismatch: expected {}, emitted {}",
            expected.len(),
            actual.len()
        )));
    }
    Ok(())
}
