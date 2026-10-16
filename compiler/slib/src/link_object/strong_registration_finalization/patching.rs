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
    }
    Ok(())
}

pub(super) fn patch_types<D, C>(
    fingerprints: &TypeFingerprintSetV1<D, C>,
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
    }
    Ok(())
}

pub(super) fn patch_static_storages(
    fingerprints: &StaticStorageFingerprintSetV1,
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
    }
    Ok(())
}

pub(super) fn patch_initializations<I>(
    fingerprints: &InitializationFingerprintSetV1<I>,
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
        if let Some(patch) = verified.gateway_definition_patch() {
            let index = object_indexes
                .get(&patch.member())
                .copied()
                .ok_or(StrongRegistrationPatchError::MissingObject(patch.member()))?;
            let bytes = &mut objects[index].1;
            let digest = computed.gateway_definition().ok_or(
                StrongRegistrationPatchError::MissingGatewayDigest(plan.semantic().unit()),
            )?;
            write_digest(
                bytes,
                patch,
                digest.as_array(),
                owner,
                StrongRegistrationPatchFieldV1::GatewayDefinition,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::link_object) fn patch_initializations_for_test(
    fingerprints: &InitializationFingerprintSetV1,
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
    patch: DigestPatchSiteV1,
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
    slot.copy_from_slice(digest);
    Ok(())
}
