use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    PersistentCallableBodyId, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentSafepointSiteId, PersistentStaticStorageId,
};
use scoop_wire::sha256;

use super::initialization_registrations::record::expected_final_record as expected_final_initialization_record;
use super::safepoint_registrations::physical::{validate_objects, verified_member};
use super::safepoint_registrations::record::expected_final_record as expected_final_safepoint_record;
use super::static_storage_registrations::record::expected_final_record as expected_final_static_storage_record;
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointFingerprintSetV1};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectCandidateV1, ScoopLirObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedMaterializedPatchSiteV1, VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongImmortalObjectFingerprintSetV1, VerifiedStrongInitializationFingerprintSetV1,
    VerifiedStrongStaticStorageFingerprintSetV1, VerifiedStrongTypeFingerprintSetV1,
    callable_registrations::record::expected_final_record as expected_final_callable_record,
    immortal_registrations::record::expected_final_record as expected_final_immortal_object_record,
    type_registrations::record::expected_final_record as expected_final_type_record,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
};

const DIGEST_WIDTH: usize = 32;

/// One copied Scoop LIR object whose verified strong-registration digest slots
/// have been patched and whose final Mach-O envelope has been revalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn envelope(&self) -> &ValidatedScoopLirObjectEnvelopeV1 {
        &self.envelope
    }
}

/// Typed result of applying every verified strong-registration digest to
/// copied object bytes. This is not yet the full digest-graph finalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongRegistrationPatchSetV1 {
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
    objects: Vec<VerifiedStrongRegistrationPatchedScoopLirObjectV1>,
}

impl VerifiedStrongRegistrationPatchSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.safepoints.producer()
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeFingerprintSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &VerifiedStrongImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &VerifiedStrongStaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationFingerprintSetV1 {
        &self.initializations
    }

    pub fn objects(&self) -> &[VerifiedStrongRegistrationPatchedScoopLirObjectV1] {
        &self.objects
    }
}

/// Copy the exact verified provisional objects, write every declared strong
/// registration slot, and revalidate normalized content and shape.
pub fn patch_strong_registration_fingerprints_v1(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongRegistrationPatchSetV1, StrongRegistrationPatchError> {
    let safepoint_registrations = safepoints.registrations();
    let callable_body_objects = callables.body_objects();
    let callable_registrations = callable_body_objects.registration_objects().registrations();
    let type_registrations = types.dependencies().registration_objects().registrations();
    let immortal_registrations = immortal_objects
        .object_definitions()
        .registration_objects()
        .registrations();
    let static_storage_registrations = static_storages
        .shapes()
        .storage_definitions()
        .registration_objects()
        .registrations();
    let initialization_registrations = initializations
        .definitions()
        .registration_objects()
        .registrations();
    if safepoint_registrations.patch_sites() != callable_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != type_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != immortal_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != static_storage_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != initialization_registrations.patch_sites()
        || safepoint_registrations.stackmaps() != callable_body_objects.stackmaps()
    {
        return Err(StrongRegistrationPatchError::ProofMismatch);
    }
    let builtins = safepoint_registrations.stackmaps().builtins();
    validate_objects(builtins, scoop_objects)
        .map_err(StrongRegistrationPatchError::ObjectValidation)?;
    validate_proof_coverage(
        &safepoints,
        &callables,
        &types,
        &immortal_objects,
        &static_storages,
        &initializations,
    )?;

    let mut objects = scoop_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let object_indexes = objects
        .iter()
        .enumerate()
        .map(|(index, (member, _))| (*member, index))
        .collect::<BTreeMap<_, _>>();

    patch_safepoints(&safepoints, &object_indexes, &mut objects)?;
    patch_callables(&callables, &object_indexes, &mut objects)?;
    patch_types(&types, &object_indexes, &mut objects)?;
    patch_immortal_objects(&immortal_objects, &object_indexes, &mut objects)?;
    patch_static_storages(&static_storages, &object_indexes, &mut objects)?;
    patch_initializations(&initializations, &object_indexes, &mut objects)?;

    let mut finalized = Vec::with_capacity(objects.len());
    for (member, bytes) in objects {
        verify_normalized_content(
            member,
            &bytes,
            &safepoints,
            &callables,
            &types,
            &immortal_objects,
            &static_storages,
            &initializations,
        )?;
        let envelope = validate_scoop_lir_llvm_22_1_object_envelope_v1(&bytes)
            .map_err(|source| StrongRegistrationPatchError::FinalEnvelope { member, source })?;
        let original = verified_member(builtins, member)
            .map_err(StrongRegistrationPatchError::ObjectValidation)?
            .definitions()
            .sections();
        if !same_macho_shape(original, envelope.sections()) {
            return Err(StrongRegistrationPatchError::MachOShapeChanged(member));
        }
        finalized.push(VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
            member,
            bytes,
            envelope,
        });
    }

    Ok(VerifiedStrongRegistrationPatchSetV1 {
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        objects: finalized,
    })
}

fn validate_proof_coverage(
    safepoints: &VerifiedStrongSafepointFingerprintSetV1,
    callables: &VerifiedStrongCallableFingerprintSetV1,
    types: &VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: &VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: &VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: &VerifiedStrongInitializationFingerprintSetV1,
) -> Result<(), StrongRegistrationPatchError> {
    let verified_safepoints = safepoints.registrations().registrations();
    if verified_safepoints.len() != safepoints.registrations().plan().registrations().len()
        || verified_safepoints.len() != safepoints.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let verified_callables = callables
        .body_objects()
        .registration_objects()
        .registrations()
        .registrations();
    if verified_callables.len()
        != callables
            .body_objects()
            .registration_objects()
            .registrations()
            .plan()
            .registrations()
            .len()
        || verified_callables.len() != callables.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let type_registrations = types.dependencies().registration_objects().registrations();
    if type_registrations.registrations().len() != type_registrations.plan().registrations().len()
        || type_registrations.registrations().len() != types.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let immortal_registrations = immortal_objects
        .object_definitions()
        .registration_objects()
        .registrations();
    if immortal_registrations.registrations().len()
        != immortal_registrations.plan().registrations().len()
        || immortal_registrations.registrations().len() != immortal_objects.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let static_storage_registrations = static_storages
        .shapes()
        .storage_definitions()
        .registration_objects()
        .registrations();
    if static_storage_registrations.registrations().len()
        != static_storage_registrations.plan().registrations().len()
        || static_storage_registrations.registrations().len()
            != static_storages
                .shapes()
                .storage_definitions()
                .registration_objects()
                .fingerprints()
                .len()
        || static_storage_registrations.registrations().len()
            != static_storages
                .shapes()
                .storage_definitions()
                .fingerprints()
                .len()
        || static_storage_registrations.registrations().len()
            != static_storages.shapes().fingerprints().len()
        || static_storage_registrations.registrations().len()
            != static_storages.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let initialization_definitions = initializations.definitions();
    let initialization_registration_objects = initialization_definitions.registration_objects();
    let initialization_registrations = initialization_registration_objects.registrations();
    let initialization_count = initialization_registrations.registrations().len();
    if initialization_count != initialization_registrations.plan().registrations().len()
        || initialization_count != initialization_registration_objects.fingerprints().len()
        || initialization_count != initialization_definitions.fingerprints().len()
        || initialization_count != initializations.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    for computed in initializations.fingerprints() {
        let gateway = (
            computed.gateway_body(),
            computed.gateway_definition_node(),
            computed.gateway_definition(),
        );
        let (Some(body), Some(node), Some(definition)) = gateway else {
            if gateway == (None, None, None) {
                continue;
            }
            return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
        };
        let Some(callable) = callables
            .fingerprints()
            .iter()
            .find(|candidate| candidate.body() == body)
        else {
            return Err(StrongRegistrationPatchError::ProofMismatch);
        };
        if callable.body_definition_node() != node || callable.body_definition() != definition {
            return Err(StrongRegistrationPatchError::ProofMismatch);
        }
    }
    Ok(())
}

fn patch_safepoints(
    fingerprints: &VerifiedStrongSafepointFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    for ((verified, plan), computed) in fingerprints
        .registrations()
        .registrations()
        .iter()
        .zip(fingerprints.registrations().plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::Safepoint(plan.site());
        if verified.site() != plan.site() || verified.site() != computed.site() {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.normalized_stackmap_patch(),
            computed.stackmap().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::NormalizedStackmap,
        )?;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_safepoint(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

fn patch_callables(
    fingerprints: &VerifiedStrongCallableFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints
        .body_objects()
        .registration_objects()
        .registrations();
    for ((verified, plan), computed) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::Callable(plan.body());
        if verified.body() != plan.body() || verified.body() != computed.body() {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.body_definition_patch(),
            computed.body_definition().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::CallableBodyDefinition,
        )?;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_callable(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

fn patch_types(
    fingerprints: &VerifiedStrongTypeFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints
        .dependencies()
        .registration_objects()
        .registrations();
    for ((verified, plan), computed) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::Type(plan.exact_type());
        if verified.exact_type() != plan.exact_type()
            || verified.exact_type() != computed.exact_type()
        {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.descriptor_definition_patch(),
            computed.descriptor_definition().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::DescriptorDefinition,
        )?;
        write_digest(
            bytes,
            verified.layout_fingerprint_patch(),
            computed.layout().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::Layout,
        )?;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_type(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

fn patch_immortal_objects(
    fingerprints: &VerifiedStrongImmortalObjectFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints
        .object_definitions()
        .registration_objects()
        .registrations();
    for ((verified, plan), computed) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::ImmortalObject(plan.object());
        if verified.object() != plan.object() || verified.object() != computed.object() {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_immortal_object(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

fn patch_static_storages(
    fingerprints: &VerifiedStrongStaticStorageFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints
        .shapes()
        .storage_definitions()
        .registration_objects()
        .registrations();
    for ((verified, plan), computed) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::StaticStorage(plan.semantic().storage());
        if verified.storage() != plan.semantic().storage()
            || verified.storage() != computed.storage()
        {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.scan_fingerprint_patch(),
            computed.scan().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::Scan,
        )?;
        write_digest(
            bytes,
            verified.layout_fingerprint_patch(),
            computed.layout().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::Layout,
        )?;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_static_storage(bytes, verified, plan, computed)?;
    }
    Ok(())
}

fn patch_initializations(
    fingerprints: &VerifiedStrongInitializationFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints
        .definitions()
        .registration_objects()
        .registrations();
    for ((verified, plan), computed) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(fingerprints.fingerprints())
    {
        let owner = StrongRegistrationPatchOwnerV1::InitializationUnit(plan.semantic().unit());
        if verified.unit() != plan.semantic().unit() || verified.unit() != computed.unit() {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        let index = object_indexes.get(&verified.member()).copied().ok_or(
            StrongRegistrationPatchError::MissingObject(verified.member()),
        )?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            owner,
            StrongRegistrationPatchFieldV1::RegistrationDefinition,
        )?;
        let gateway_patch = verified.gateway_definition_patch();
        let gateway_definition = computed.gateway_definition();
        if let (Some(patch), Some(digest)) = (gateway_patch, gateway_definition) {
            write_digest(
                bytes,
                patch,
                digest.as_array(),
                owner,
                StrongRegistrationPatchFieldV1::GatewayDefinition,
            )?;
        } else if gateway_patch.is_some() || gateway_definition.is_some() {
            return Err(StrongRegistrationPatchError::ProofOwnerMismatch { owner });
        }
        validate_final_initialization(bytes, verified.checked_offset(), plan, computed)?;
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::link_object) fn patch_initializations_for_test(
    fingerprints: &VerifiedStrongInitializationFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<Vec<(SlibMemberId, Vec<u8>)>, StrongRegistrationPatchError> {
    let mut objects = scoop_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let object_indexes = objects
        .iter()
        .enumerate()
        .map(|(index, (member, _))| (*member, index))
        .collect::<BTreeMap<_, _>>();
    patch_initializations(fingerprints, &object_indexes, &mut objects)?;
    Ok(objects)
}

fn write_digest(
    object: &mut [u8],
    patch: VerifiedMaterializedPatchSiteV1,
    digest: &[u8; DIGEST_WIDTH],
    owner: StrongRegistrationPatchOwnerV1,
    field: StrongRegistrationPatchFieldV1,
) -> Result<(), StrongRegistrationPatchError> {
    let start = usize::try_from(patch.checked_offset())
        .map_err(|_| StrongRegistrationPatchError::PatchRange { owner, field })?;
    let end = start
        .checked_add(DIGEST_WIDTH)
        .ok_or(StrongRegistrationPatchError::PatchRange { owner, field })?;
    let slot = object
        .get_mut(start..end)
        .ok_or(StrongRegistrationPatchError::PatchRange { owner, field })?;
    if let Some(offset) = slot.iter().position(|byte| *byte != 0) {
        return Err(StrongRegistrationPatchError::NonZeroPatchSlot {
            owner,
            field,
            offset: u8::try_from(offset).expect("digest width fits u8"),
        });
    }
    slot.copy_from_slice(digest);
    Ok(())
}

fn validate_final_safepoint(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    computed: &super::VerifiedStrongSafepointFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_safepoint_record(
        plan,
        computed.registration().as_array(),
        computed.stackmap().as_array(),
    );
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Safepoint(plan.site()),
    )
}

fn validate_final_callable(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongCallableFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_callable_record(
        plan,
        computed.registration().as_array(),
        computed.body_definition().as_array(),
    );
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Callable(plan.body()),
    )
}

fn validate_final_type(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongTypeRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongTypeFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_type_record(
        plan,
        computed.registration().as_array(),
        computed.descriptor_definition().as_array(),
        computed.layout().as_array(),
    );
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Type(plan.exact_type()),
    )
}

fn validate_final_immortal_object(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongImmortalObjectRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongImmortalObjectFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_immortal_object_record(plan, computed.registration().as_array());
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::ImmortalObject(plan.object()),
    )
}

fn validate_final_static_storage(
    object: &[u8],
    verified: &crate::link_object::VerifiedStrongStaticStorageRegistrationV1,
    plan: &scoop_lir::StrongStaticStorageRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongStaticStorageFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_static_storage_record(
        plan,
        verified.template_relocation().encoded_value(),
        verified.relocation_table_relocation().encoded_value(),
        computed.registration().as_array(),
        computed.scan().as_array(),
        computed.layout().as_array(),
    );
    validate_final_record(
        object,
        verified.checked_offset(),
        &expected,
        StrongRegistrationPatchOwnerV1::StaticStorage(plan.semantic().storage()),
    )
}

fn validate_final_initialization(
    object: &[u8],
    checked_offset: u64,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongInitializationFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let gateway_definition = computed.gateway_definition();
    let gateway = gateway_definition
        .as_ref()
        .map(|fingerprint| fingerprint.as_array());
    let expected =
        expected_final_initialization_record(plan, computed.registration().as_array(), gateway);
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::InitializationUnit(plan.semantic().unit()),
    )
}

fn validate_final_record(
    object: &[u8],
    checked_offset: u64,
    expected: &[u8],
    owner: StrongRegistrationPatchOwnerV1,
) -> Result<(), StrongRegistrationPatchError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongRegistrationPatchError::RecordRange(owner))?;
    let end = start
        .checked_add(expected.len())
        .ok_or(StrongRegistrationPatchError::RecordRange(owner))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongRegistrationPatchError::RecordRange(owner))?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(StrongRegistrationPatchError::FinalRecordMismatch {
            owner,
            offset: u16::try_from(offset).expect("descriptor offset fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_normalized_content(
    member: SlibMemberId,
    final_bytes: &[u8],
    safepoints: &VerifiedStrongSafepointFingerprintSetV1,
    callables: &VerifiedStrongCallableFingerprintSetV1,
    types: &VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: &VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: &VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: &VerifiedStrongInitializationFingerprintSetV1,
) -> Result<(), StrongRegistrationPatchError> {
    let mut normalized = final_bytes.to_vec();
    for registration in safepoints
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.registration_definition_patch(),
                registration.normalized_stackmap_patch(),
            ],
        )?;
    }
    for registration in callables
        .body_objects()
        .registration_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.registration_definition_patch(),
                registration.body_definition_patch(),
            ],
        )?;
    }
    for registration in types
        .dependencies()
        .registration_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.registration_definition_patch(),
                registration.descriptor_definition_patch(),
                registration.layout_fingerprint_patch(),
            ],
        )?;
    }
    for registration in immortal_objects
        .object_definitions()
        .registration_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [registration.registration_definition_patch()],
        )?;
    }
    for registration in static_storages
        .shapes()
        .storage_definitions()
        .registration_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.registration_definition_patch(),
                registration.scan_fingerprint_patch(),
                registration.layout_fingerprint_patch(),
            ],
        )?;
    }
    for registration in initializations
        .definitions()
        .registration_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slot(
            &mut normalized,
            member,
            registration.registration_definition_patch(),
        )?;
        if let Some(patch) = registration.gateway_definition_patch() {
            zero_digest_slot(&mut normalized, member, patch)?;
        }
    }
    let builtins = safepoints.registrations().stackmaps().builtins();
    let original = verified_member(builtins, member)
        .map_err(StrongRegistrationPatchError::ObjectValidation)?
        .definitions()
        .sections()
        .envelope();
    if u64::try_from(normalized.len()).ok() != Some(original.byte_length())
        || sha256(&normalized) != original.content_digest()
    {
        return Err(StrongRegistrationPatchError::NormalizedObjectMismatch(
            member,
        ));
    }
    Ok(())
}

fn zero_digest_slots<const N: usize>(
    bytes: &mut [u8],
    member: SlibMemberId,
    patches: [VerifiedMaterializedPatchSiteV1; N],
) -> Result<(), StrongRegistrationPatchError> {
    for patch in patches {
        let start = usize::try_from(patch.checked_offset())
            .map_err(|_| StrongRegistrationPatchError::NormalizedObjectRange(member))?;
        let end = start
            .checked_add(DIGEST_WIDTH)
            .ok_or(StrongRegistrationPatchError::NormalizedObjectRange(member))?;
        bytes
            .get_mut(start..end)
            .ok_or(StrongRegistrationPatchError::NormalizedObjectRange(member))?
            .fill(0);
    }
    Ok(())
}

fn zero_digest_slot(
    bytes: &mut [u8],
    member: SlibMemberId,
    patch: VerifiedMaterializedPatchSiteV1,
) -> Result<(), StrongRegistrationPatchError> {
    zero_digest_slots(bytes, member, [patch])
}

pub(in crate::link_object) fn same_macho_shape(
    original: &ValidatedBuiltinObjectSectionInventoryV1,
    final_inventory: &ValidatedBuiltinObjectSectionInventoryV1,
) -> bool {
    let original_envelope = original.envelope();
    let final_envelope = final_inventory.envelope();
    original.profile() == final_inventory.profile()
        && original.roles() == final_inventory.roles()
        && original_envelope.byte_length() == final_envelope.byte_length()
        && original_envelope.load_command_count() == final_envelope.load_command_count()
        && original_envelope.section_count() == final_envelope.section_count()
        && original_envelope.symbol_count() == final_envelope.symbol_count()
        && original_envelope.relocation_count() == final_envelope.relocation_count()
        && original_envelope.deployment() == final_envelope.deployment()
        && original_envelope.sections() == final_envelope.sections()
        && original_envelope.symbols() == final_envelope.symbols()
        && original_envelope.relocations() == final_envelope.relocations()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchOwnerV1 {
    Safepoint(PersistentSafepointSiteId),
    Callable(PersistentCallableBodyId),
    Type(PersistentExactTypeId),
    ImmortalObject(PersistentImmortalObjectId),
    StaticStorage(PersistentStaticStorageId),
    InitializationUnit(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchFieldV1 {
    RegistrationDefinition,
    NormalizedStackmap,
    CallableBodyDefinition,
    DescriptorDefinition,
    Layout,
    Scan,
    GatewayDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchError {
    ProofMismatch,
    ObjectValidation(StrongSafepointRegistrationValidationError),
    ProofCoverageMismatch,
    ProofOwnerMismatch {
        owner: StrongRegistrationPatchOwnerV1,
    },
    MissingObject(SlibMemberId),
    PatchRange {
        owner: StrongRegistrationPatchOwnerV1,
        field: StrongRegistrationPatchFieldV1,
    },
    NonZeroPatchSlot {
        owner: StrongRegistrationPatchOwnerV1,
        field: StrongRegistrationPatchFieldV1,
        offset: u8,
    },
    RecordRange(StrongRegistrationPatchOwnerV1),
    FinalRecordMismatch {
        owner: StrongRegistrationPatchOwnerV1,
        offset: u16,
        expected: u8,
        actual: u8,
    },
    NormalizedObjectRange(SlibMemberId),
    NormalizedObjectMismatch(SlibMemberId),
    FinalEnvelope {
        member: SlibMemberId,
        source: ScoopLirObjectEnvelopeValidationError,
    },
    MachOShapeChanged(SlibMemberId),
}

impl fmt::Display for StrongRegistrationPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to patch strong registration digests: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::FinalEnvelope { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
