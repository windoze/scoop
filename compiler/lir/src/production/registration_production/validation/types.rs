//! Complete type registration validation.

use super::type_references::{
    self, DependencyTypeReferences, LegacyTypeReferences, TypeReferences,
};
use super::*;
use crate::{
    DecodedStrongTypeRegistrationPlan, StrongDescriptorReference, StrongTypeDescriptorSemanticPlan,
    StrongTypeDescriptorSemanticPlanSet,
};
use scoop_wire::{WirePath, encode_canonical_temporary};

type DecodedTypeFor<R> = DecodedStrongTypeRegistrationPlan<
    <R as TypeReferences>::Parent,
    <R as TypeReferences>::DecodedDescriptor,
    <R as TypeReferences>::DecodedCallable,
>;
type SemanticTypeSetFor<R> = StrongTypeDescriptorSemanticPlanSet<
    <R as TypeReferences>::Descriptor,
    <R as TypeReferences>::Callable,
>;

pub(crate) fn validate_types(
    decoded: Vec<DecodedStrongTypeRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,

    digests: &DigestFinalizationPlanV1,
) -> Result<StrongTypeDescriptorSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    validate_types_with_references(
        decoded,
        target,
        foundation,
        identities,
        digests,
        &LegacyTypeReferences {
            foundation,
            identities,
        },
    )
}

/// Replays the complete type-registration table and binds dependency references
/// to physical definitions. This constituent does not authorize exports or
/// selections; the production reader must additionally join the same records
/// to the complete layout/ABI section and its committed dependency closure.
#[allow(clippy::too_many_arguments)]
pub fn validate_type_registration_constituents_v2(
    decoded: Vec<crate::DecodedStrongTypeRegistrationPlanV2>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,

    definitions: &crate::StrongTypeReferenceDefinitionsV2,
    digests: &DigestFinalizationPlanV1,
) -> Result<crate::StrongTypeDescriptorSemanticPlanSetV2, StrongRegistrationProductionValidationError>
{
    let actual = definitions.consumer();
    if actual != foundation.producer() {
        return Err(
            crate::StrongTypeReferenceResolutionErrorV2::ProducerMismatch {
                expected: foundation.producer(),
                actual,
            }
            .into(),
        );
    }
    validate_types_with_references(
        decoded,
        target,
        foundation,
        identities,
        digests,
        &DependencyTypeReferences {
            local: LegacyTypeReferences {
                foundation,
                identities,
            },
            definitions,
        },
    )
}

fn validate_types_with_references<R: TypeReferences>(
    decoded: Vec<DecodedTypeFor<R>>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    digests: &DigestFinalizationPlanV1,
    references: &R,
) -> Result<SemanticTypeSetFor<R>, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::Type,
        decoded.len(),
        identities.type_registrations().len(),
    )?;
    let path = WirePath::root();

    let mut actual = Vec::new();
    scoop_wire::allocation::try_reserve(&mut actual, decoded.len(), &path)?;
    for record in &decoded {
        actual.push(encode_canonical_temporary(record, &path)?);
    }
    let mut descriptors = Vec::new();
    scoop_wire::allocation::try_reserve(&mut descriptors, decoded.len(), &path)?;
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
        let parent = references.parent(decoded.parent, index)?;
        let vtable =
            type_references::vtable(decoded.vtable, exact_type, foundation, references, index)?;
        let mut table_ids = BTreeSet::from([vtable.table()]);
        let mut interfaces = BTreeSet::new();
        let mut itables = Vec::new();
        scoop_wire::allocation::try_reserve(&mut itables, decoded.itables.len(), &path)?;
        for decoded_itable in decoded.itables {
            let itable =
                type_references::itable(decoded_itable, exact_type, foundation, references, index)?;
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
        descriptors.push(StrongTypeDescriptorSemanticPlan::from_artifact(
            exact_type,
            decoded.diagnostic_name,
            layout,
            scan,
            instance_shape,
            inline_scan,
            parent,
            vtable,
            itables,
            decoded
                .relations
                .try_map(|reference| references.parent(reference, index))?,
        ));
    }
    let semantics = StrongTypeDescriptorSemanticPlanSet::from_artifact(
        foundation.producer(),
        target.wire_id(),
        descriptors,
    );
    let expected = crate::StrongTypeRegistrationPlanSet::new(
        target, foundation, identities, &semantics, digests,
    )
    .map_err(|error| {
        StrongRegistrationProductionValidationError::Expected(Box::new(
            StrongRegistrationProductionBuildError::Types(error),
        ))
    })?;
    for (index, (actual, expected)) in actual.iter().zip(expected.registrations()).enumerate() {
        let expected = encode_canonical_temporary(expected, &path)?;
        if actual != &expected {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table: RegistrationProductionTableV1::Type,
                index,
            });
        }
    }
    Ok(semantics)
}
