use std::collections::BTreeSet;

use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::targets::TargetData;
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentSymbolRequest, StrongProductionSectionV1,
};

use super::{
    EmittedEntryProductionV1, emit_cone_image_v1, emit_entry_production_v1,
    emit_strong_callable_registrations_v1, emit_strong_immortal_object_registrations_v1,
    emit_strong_initialization_unit_registrations_v1, emit_strong_safepoint_registrations_v1,
    emit_strong_static_storage_registrations_v1, emit_strong_type_registrations_v1,
};
use crate::CodegenError;

/// Object-independent location of one provisional digest slot emitted by
/// codegen. The packager resolves the typed owner symbol and offset against
/// the verified object; it never rediscovers a slot from descriptor bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalStrongDigestPatchLocationV1 {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: PersistentSymbolRequest,
    offset_within_owner: u64,
    width_bytes: u8,
}

impl ProvisionalStrongDigestPatchLocationV1 {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn owner(self) -> PersistentSymbolRequest {
        self.owner
    }

    pub const fn offset_within_owner(self) -> u64 {
        self.offset_within_owner
    }

    pub const fn width_bytes(self) -> u8 {
        self.width_bytes
    }
}

/// Complete provisional runtime-metadata result for one strong Cone.
///
/// LLVM values deliberately do not escape this product. The only data needed
/// by object packaging is the canonical, typed patch-location set below.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmittedStrongRuntimeMetadataV1 {
    producer: ConeIdentity,
    patch_locations: Vec<ProvisionalStrongDigestPatchLocationV1>,
}

impl EmittedStrongRuntimeMetadataV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn patch_locations(&self) -> &[ProvisionalStrongDigestPatchLocationV1] {
        &self.patch_locations
    }
}

/// Emit the complete runtime-metadata surface described by one closed strong
/// production section and return its exact provisional digest locations.
///
/// The caller owns the LLVM module under construction. On failure that module
/// must be discarded; no partially emitted module is a successful product.
pub(crate) fn emit_strong_runtime_metadata_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    production: &StrongProductionSectionV1,
) -> Result<EmittedStrongRuntimeMetadataV1, CodegenError> {
    let registrations = production.registration_production();
    let safepoints =
        emit_strong_safepoint_registrations_v1(context, llvm, registrations.safepoints())?;
    let callables =
        emit_strong_callable_registrations_v1(context, llvm, registrations.callables())?;
    let types = emit_strong_type_registrations_v1(context, llvm, registrations.types())?;
    let immortal_objects = emit_strong_immortal_object_registrations_v1(
        context,
        llvm,
        registrations.immortal_objects(),
    )?;
    let static_storages = emit_strong_static_storage_registrations_v1(
        context,
        llvm,
        target_data,
        registrations.static_storages(),
    )?;
    let initialization_units = emit_strong_initialization_unit_registrations_v1(
        context,
        llvm,
        registrations.initialization_units(),
    )?;
    let entry = emit_entry_production_v1(context, llvm, production.entry_plan())?;
    let image = emit_cone_image_v1(context, llvm, production.image_plan())?;

    let producer = production.external_bridges().producer();
    for actual in [
        safepoints.producer(),
        callables.producer(),
        types.producer(),
        immortal_objects.producer(),
        static_storages.producer(),
        initialization_units.producer(),
        production.image_plan().cone().identity(),
    ] {
        if actual != producer {
            return Err(CodegenError(format!(
                "strong runtime metadata producer mismatch: expected {producer}, found {actual}"
            )));
        }
    }

    let mut patches = Vec::new();
    for registration in safepoints.registrations() {
        let registration = *registration;
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
        record_patch(
            production,
            &mut patches,
            registration.normalized_stackmap_patch().into_parts(),
        )?;
    }
    for registration in callables.registrations() {
        let registration = *registration;
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
        record_patch(
            production,
            &mut patches,
            registration.body_definition_patch().into_parts(),
        )?;
    }
    for registration in types.registrations() {
        let registration = *registration;
        for patch in [
            registration.registration_definition_patch(),
            registration.descriptor_definition_patch(),
            registration.layout_fingerprint_patch(),
        ] {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    for registration in immortal_objects.registrations() {
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
    }
    for registration in static_storages.registrations() {
        let registration = *registration;
        for patch in [
            registration.registration_definition_patch(),
            registration.scan_fingerprint_patch(),
            registration.layout_fingerprint_patch(),
        ] {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    for registration in initialization_units.registrations() {
        let registration = *registration;
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
        if let Some(patch) = registration.gateway_definition_patch() {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    if let EmittedEntryProductionV1::Executable(entry) = entry {
        record_patch(
            production,
            &mut patches,
            PatchParts::without_atom(entry.source_signature_patch()),
        )?;
        record_patch(
            production,
            &mut patches,
            PatchParts::without_atom(entry.gateway_definition_patch()),
        )?;
    }
    record_patch(production, &mut patches, image.patch().into_parts())?;

    patches.sort_unstable_by_key(|patch| patch.intent);
    validate_patch_coverage(production, &patches)?;
    Ok(EmittedStrongRuntimeMetadataV1 {
        producer,
        patch_locations: patches,
    })
}

#[derive(Clone, Copy)]
struct PatchParts<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: Option<ObjectDefinitionAtomId>,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
    byte_size: u64,
}

impl<'ctx> PatchParts<'ctx> {
    fn without_atom(patch: super::RootEntryPatchSiteV1<'ctx>) -> Self {
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

trait PatchSiteParts<'ctx> {
    fn into_parts(self) -> PatchParts<'ctx>;
}

macro_rules! impl_patch_site_parts {
    ($($ty:ident),+ $(,)?) => {$(
        impl<'ctx> PatchSiteParts<'ctx> for super::$ty<'ctx> {
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

fn record_patch(
    production: &StrongProductionSectionV1,
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

fn validate_patch_coverage(
    production: &StrongProductionSectionV1,
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

#[cfg(test)]
mod tests;
