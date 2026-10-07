use super::*;

pub(super) fn patch_safepoints(
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

        validate_final_safepoint(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

pub(super) fn patch_callables(
    fingerprints: &VerifiedStrongCallableFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints.body_objects().registrations();
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

        validate_final_callable(bytes, verified.checked_offset(), *plan, computed)?;
    }
    Ok(())
}

pub(super) fn patch_types<D, C>(
    fingerprints: &VerifiedStrongTypeFingerprintSetV1<D, C>,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let registrations = fingerprints.registrations();
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

        validate_final_type(bytes, verified.checked_offset(), plan, computed)?;
    }
    Ok(())
}

pub(super) fn patch_immortal_objects(
    fingerprints: &VerifiedStrongImmortalObjectFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints.registrations();
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

        validate_final_immortal_object(bytes, verified.checked_offset(), *plan)?;
    }
    Ok(())
}

pub(super) fn patch_static_storages(
    fingerprints: &VerifiedStrongStaticStorageFingerprintSetV1,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints.shapes().registrations();
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

        validate_final_static_storage(bytes, verified, plan, computed)?;
    }
    Ok(())
}

pub(super) fn patch_initializations<I>(
    fingerprints: &VerifiedStrongInitializationFingerprintSetV1<I>,
    object_indexes: &BTreeMap<SlibMemberId, usize>,
    objects: &mut [(SlibMemberId, Vec<u8>)],
) -> Result<(), StrongRegistrationPatchError> {
    let registrations = fingerprints.registrations();
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

pub(super) fn write_digest(
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
