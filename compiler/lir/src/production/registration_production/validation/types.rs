//! Complete type registration validation.

use super::*;

pub(crate) fn validate_types(
    decoded: Vec<DecodedStrongTypeRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    external_bridges: &StrongExternalLirBridgeSurfaceV1,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<StrongTypeDescriptorSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::Type,
        decoded.len(),
        identities.type_registrations().len(),
    )?;
    let actual = decoded
        .iter()
        .map(encode)
        .collect::<Result<Vec<_>, _>>()
        .map_err(StrongRegistrationProductionValidationError::Encode)?;
    let mut descriptors = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.type_registrations())
        .enumerate()
    {
        let exact_type = verify_expected(
            decoded.exact_type,
            identity.semantic_id(),
            RegistrationProductionTableV1::Type,
            index,
            "exact_type",
        )?;
        if decoded.diagnostic_name.is_empty() {
            return Err(semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "diagnostic_name",
            ));
        }
        let layout_record = foundation
            .layouts()
            .iter()
            .find(|record| record.id().as_array() == decoded.layout.as_array())
            .ok_or_else(|| {
                semantic_error(
                    RegistrationProductionTableV1::Type,
                    index,
                    "instance_layout",
                )
            })?;
        let layout = layout_record.id();
        if layout_record.key().exact_type() != exact_type
            || layout_record.key().target_profile() != &target.wire_id()
            || layout_record.key().representation() != RepresentationRole::ManagedObject
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "instance_layout_relation",
            ));
        }
        let scan_record = foundation
            .scans()
            .iter()
            .find(|record| record.id().as_array() == decoded.instance_scan.as_array())
            .ok_or_else(|| {
                semantic_error(RegistrationProductionTableV1::Type, index, "instance_scan")
            })?;
        let scan = scan_record.id();
        if scan_record.key().layout() != layout
            || scan_record.key().role() != ScanRole::ManagedObject
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "instance_scan_relation",
            ));
        }
        let instance_shape = validate_type_instance_shape(decoded.instance_shape, target, index)?;
        let inline_scan = validate_type_descriptor_inline_scan(
            decoded.inline_scan,
            &instance_shape,
            exact_type,
            target,
            foundation,
            index,
        )?;
        let parent = validate_optional_type_descriptor_ref(
            decoded.parent,
            identities,
            external_bridges,
            index,
            "parent",
        )?;
        let vtable = validate_type_vtable(
            decoded.vtable,
            exact_type,
            foundation,
            external_bridges,
            index,
        )?;
        let mut table_ids = BTreeSet::from([vtable.table()]);
        let mut interfaces = BTreeSet::new();
        let mut itables = Vec::with_capacity(decoded.itables.len());
        for decoded_itable in decoded.itables {
            let itable = validate_type_itable(
                decoded_itable,
                exact_type,
                identities,
                foundation,
                external_bridges,
                index,
            )?;
            if !table_ids.insert(itable.table()) {
                return Err(semantic_error(
                    RegistrationProductionTableV1::Type,
                    index,
                    "duplicate_dispatch_table",
                ));
            }
            if !interfaces.insert(itable.interface().exact_type()) {
                return Err(semantic_error(
                    RegistrationProductionTableV1::Type,
                    index,
                    "duplicate_interface",
                ));
            }
            itables.push(itable);
        }
        descriptors.push(StrongTypeDescriptorSemanticPlanV1::from_artifact(
            exact_type,
            decoded.diagnostic_name,
            layout,
            scan,
            instance_shape,
            inline_scan,
            parent,
            vtable,
            itables,
        ));
    }
    let semantics = StrongTypeDescriptorSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        target.wire_id(),
        descriptors,
    );
    let expected = crate::StrongTypeRegistrationPlanSetV1::new(
        target, foundation, identities, &semantics, digests,
    )
    .map_err(|error| {
        StrongRegistrationProductionValidationError::Expected(Box::new(
            StrongRegistrationProductionBuildError::Types(error),
        ))
    })?;
    for (index, (actual, expected)) in actual.iter().zip(expected.registrations()).enumerate() {
        let expected =
            encode(expected).map_err(StrongRegistrationProductionValidationError::Encode)?;
        if actual != &expected {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table: RegistrationProductionTableV1::Type,
                index,
            });
        }
    }
    Ok(semantics)
}
