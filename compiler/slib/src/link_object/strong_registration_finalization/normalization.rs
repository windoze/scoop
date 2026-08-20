use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_normalized_content<D, C, I>(
    member: SlibMemberId,
    final_bytes: &[u8],
    safepoints: &VerifiedStrongSafepointFingerprintSetV1,
    callables: &VerifiedStrongCallableFingerprintSetV1,
    types: &VerifiedStrongTypeFingerprintSetV1<D, C>,
    static_storages: &VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: &VerifiedStrongInitializationFingerprintSetV1<I>,
) -> Result<(), StrongRegistrationPatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
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
            [registration.normalized_stackmap_patch()],
        )?;
    }
    for registration in callables
        .body_objects()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [registration.body_definition_patch()],
        )?;
    }
    for registration in types
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.descriptor_definition_patch(),
                registration.layout_fingerprint_patch(),
            ],
        )?;
    }
    for registration in static_storages
        .shapes()
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        zero_digest_slots(
            &mut normalized,
            member,
            [
                registration.scan_fingerprint_patch(),
                registration.layout_fingerprint_patch(),
            ],
        )?;
    }
    for registration in initializations
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
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

pub(super) fn zero_digest_slots<const N: usize>(
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

pub(super) fn zero_digest_slot(
    bytes: &mut [u8],
    member: SlibMemberId,
    patch: VerifiedMaterializedPatchSiteV1,
) -> Result<(), StrongRegistrationPatchError> {
    zero_digest_slots(bytes, member, [patch])
}

pub(in crate::link_object) fn same_object_shape(
    original: &ValidatedBuiltinObjectSectionInventoryV1,
    final_inventory: &ValidatedBuiltinObjectSectionInventoryV1,
) -> bool {
    let original_envelope = original.envelope();
    let final_envelope = final_inventory.envelope();
    original.profile() == final_inventory.profile()
        && original.roles() == final_inventory.roles()
        && original_envelope.byte_length() == final_envelope.byte_length()
        && original_envelope.format() == final_envelope.format()
        && original_envelope.sections() == final_envelope.sections()
        && original_envelope.symbols() == final_envelope.symbols()
        && original_envelope.relocations() == final_envelope.relocations()
}
