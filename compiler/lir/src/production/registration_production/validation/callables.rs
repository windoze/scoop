//! Callable runtime scan and safepoint validation.

use super::*;

pub(super) fn validate_callable_runtime_scans(
    decoded: Vec<DecodedStrongCallableRuntimeScanPlanV1>,
    foundation: &OdrFreeLirFoundation,
) -> Result<StrongCallableRuntimeScanPlanSetV1, StrongRegistrationProductionValidationError> {
    let mut callables = Vec::with_capacity(decoded.len());
    for (index, decoded) in decoded.into_iter().enumerate() {
        let body = resolve_known(
            decoded.body,
            foundation
                .callable_bodies()
                .iter()
                .map(|record| record.id()),
            RegistrationProductionTableV1::Callable,
            index,
            "runtime_scan_body",
        )?;
        let mut atoms = Vec::with_capacity(decoded.atoms.len());
        for decoded_atom in decoded.atoms {
            let atom = resolve_known(
                decoded_atom.atom,
                foundation
                    .definition_atoms()
                    .iter()
                    .filter(|record| record.key().role() == DefinitionAtomRole::RuntimeRecord)
                    .map(|record| record.id()),
                RegistrationProductionTableV1::Callable,
                index,
                "runtime_scan_atom",
            )?;
            let scan = validate_callable_ref_scan(decoded_atom.scan, index)?;
            atoms.push(StrongCallableRuntimeScanAtomV1::from_artifact(atom, scan));
        }
        callables.push(StrongCallableRuntimeScanPlanV1::from_artifact(body, atoms));
    }
    StrongCallableRuntimeScanPlanSetV1::from_artifact(foundation.producer(), callables).map_err(
        |error| {
            StrongRegistrationProductionValidationError::Expected(Box::new(
                StrongRegistrationProductionBuildError::CallableRuntimeScans(error),
            ))
        },
    )
}

fn validate_callable_ref_scan(
    decoded: DecodedRefScan,
    index: usize,
) -> Result<RefScan, StrongRegistrationProductionValidationError> {
    match decoded {
        DecodedRefScan::None => Ok(RefScan::None),
        DecodedRefScan::References(offsets) => Ok(RefScan::References(offsets)),
        DecodedRefScan::Sequence(parts) => parts
            .into_iter()
            .map(|part| validate_callable_ref_scan(part, index))
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::Sequence),
        DecodedRefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let stride = NonZeroU64::new(stride).ok_or_else(|| {
                semantic_error(
                    RegistrationProductionTableV1::Callable,
                    index,
                    "runtime_scan_stride",
                )
            })?;
            let element = validate_callable_ref_scan(*element, index)?;
            let element = NonEmptyRefScan::new(element).ok_or_else(|| {
                semantic_error(
                    RegistrationProductionTableV1::Callable,
                    index,
                    "runtime_scan_element",
                )
            })?;
            Ok(RefScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element: Box::new(element),
            })
        }
    }
}

pub(super) fn validate_safepoints(
    decoded: Vec<DecodedStrongSafepointRegistrationPlanV1>,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
) -> Result<StrongSafepointSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::Safepoint,
        decoded.len(),
        identities.safepoints().len(),
    )?;
    let mut sites = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded.into_iter().zip(identities.safepoints()).enumerate()
    {
        let site = verify_expected(
            decoded.site,
            identity.semantic_id(),
            RegistrationProductionTableV1::Safepoint,
            index,
            "site",
        )?;
        let site_record = foundation
            .safepoint_sites()
            .iter()
            .find(|record| record.id() == site)
            .ok_or_else(|| {
                semantic_error(RegistrationProductionTableV1::Safepoint, index, "site")
            })?;
        let owner = verify_expected(
            decoded.owner,
            site_record.key().owner(),
            RegistrationProductionTableV1::Safepoint,
            index,
            "owner",
        )?;
        let role = site_record.key().role();
        if decoded.role != role.tag() {
            return Err(semantic_error(
                RegistrationProductionTableV1::Safepoint,
                index,
                "role",
            ));
        }
        let mapping = foundation
            .safepoint_mappings()
            .iter()
            .find(|mapping| mapping.site() == site)
            .ok_or_else(|| {
                semantic_error(RegistrationProductionTableV1::Safepoint, index, "safepoint")
            })?;
        if decoded.safepoint != mapping.safepoint().get() {
            return Err(semantic_error(
                RegistrationProductionTableV1::Safepoint,
                index,
                "safepoint",
            ));
        }
        sites.push(StrongSafepointSemanticPlanV1::from_artifact(
            site,
            mapping.safepoint(),
            owner,
            role,
            decoded.root_pair_count,
        ));
    }
    Ok(StrongSafepointSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        sites,
    ))
}
