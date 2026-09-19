use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, PersistentDispatchTableId,
    PersistentExactTypeId, RepresentationRole,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{
    ExternalStrongShapeSubjectV1, InstanceRepresentationKindV1, OdrFreeLirFoundation,
    StrongShapeDefinitionRefV1, StrongShapeDefinitionV1, StrongShapeRegistrationV1,
    StrongTypeDescriptorInlineScanPlanV1, StrongTypeDescriptorSemanticPlanV2,
    StrongTypeRegistrationPlanV2, TypeDescriptorITableDirectoryV1, TypeDescriptorInlineScanV1,
};

impl ExactDescriptorExportV1 {
    pub fn replay(
        target: crate::LirTargetProfile,
        layouts: &crate::CanonicalExactLayoutExportsV1,
        registration: &StrongTypeRegistrationPlanV2,
        diagnostics: &impl ExactTypeDiagnosticGraph,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactDescriptorError> {
        validate_registration_plan(registration, layouts, foundation, meter)?;
        let expected_registration = StrongShapeRegistrationV1::from_artifact(
            registration.exact_type(),
            registration.definition_plan(),
            registration.symbol(),
            registration.registration_fingerprint_node(),
        );
        replay_parts(
            target,
            layouts,
            registration.semantic(),
            expected_registration,
            diagnostics,
            foundation,
            meter,
        )
    }
}

pub(crate) fn validate_registration_plan(
    registration: &StrongTypeRegistrationPlanV2,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), ExactDescriptorError> {
    use scoop_identity::{DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomKey};

    let path = WirePath::root();
    meter.charge_work(layouts.records().len() as u64, &path)?;
    let exact = registration.exact_type();
    let descriptor = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        foundation,
        meter,
    )?;
    if registration.descriptor_definition_plan() != descriptor.definition()
        || registration.descriptor_primary_atom() != descriptor.primary()
        || registration.descriptor_symbol() != descriptor.symbol()
    {
        return Err(ExactDescriptorError::DescriptorPlan(exact));
    }
    let instance = layouts
        .find_exact_role(exact, RepresentationRole::ManagedObject)
        .ok_or(ExactDescriptorError::MissingInstanceLayout(exact))?;
    let layout = instance.identity().physical_definition();
    if registration.layout() != instance.identity().layout()
        || registration.layout_definition_plan() != layout.definition()
        || registration.layout_primary_atom() != layout.primary()
        || registration.layout_symbol() != layout.symbol()
    {
        return Err(ExactDescriptorError::LayoutPlan(exact));
    }
    let expected_inline = match registration.semantic().inline_scan() {
        TypeDescriptorInlineScanV1::Null => StrongTypeDescriptorInlineScanPlanV1::Null,
        TypeDescriptorInlineScanV1::Defined(scan) => {
            let physical = StrongShapeDefinitionRefV1::from_foundation(
                ExternalStrongShapeSubjectV1::Scan(scan),
                foundation,
                meter,
            )?;
            StrongTypeDescriptorInlineScanPlanV1::Defined {
                scan,
                definition_plan: physical.definition(),
            }
        }
    };
    if registration.inline_scan() != expected_inline {
        return Err(ExactDescriptorError::InlineScanPlan(exact));
    }
    let diagnostic = ObjectDefinitionAtomKey::new(
        descriptor.definition(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::ExactType(exact),
    );
    meter.charge_work(foundation.definition_atoms().len() as u64, &path)?;
    let diagnostic = foundation
        .definition_atoms()
        .iter()
        .find(|record| record.key() == &diagnostic)
        .map(|record| record.id());
    if diagnostic != Some(registration.diagnostic_atom()) {
        return Err(ExactDescriptorError::DiagnosticAtom(exact));
    }
    let directory = if registration.semantic().itables().is_empty() {
        TypeDescriptorITableDirectoryV1::Null
    } else {
        let key = ObjectDefinitionAtomKey::new(
            descriptor.definition(),
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::ExactType(exact),
        );
        meter.charge_work(foundation.definition_atoms().len() as u64, &path)?;
        let atom = foundation
            .definition_atoms()
            .iter()
            .find(|record| record.key() == &key)
            .map(|record| record.id());
        match atom {
            Some(atom) => TypeDescriptorITableDirectoryV1::Defined(atom),
            None => return Err(ExactDescriptorError::ItableDirectory(exact)),
        }
    };
    if registration.itable_directory() != directory {
        return Err(ExactDescriptorError::ItableDirectory(exact));
    }
    Ok(())
}

pub(super) fn replay_parts(
    target: crate::LirTargetProfile,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactDescriptorExportV1, ExactDescriptorError> {
    let path = WirePath::root();
    meter.charge_work(1, &path)?;
    if layouts.provider() != foundation.producer() {
        return Err(ExactDescriptorError::Provider);
    }
    if layouts.target() != target {
        return Err(ExactDescriptorError::Target);
    }
    let exact = semantic.exact_type();
    meter.charge_work((layouts.records().len() as u64).saturating_mul(2), &path)?;
    let value_record = layouts
        .find_exact_role(exact, RepresentationRole::ManagedValue)
        .ok_or(ExactDescriptorError::MissingValueLayout(exact))?;
    let instance_record = layouts
        .find_exact_role(exact, RepresentationRole::ManagedObject)
        .ok_or(ExactDescriptorError::MissingInstanceLayout(exact))?;
    let value_layout = value_record
        .value_handle()
        .ok_or_else(|| ExactDescriptorError::LayoutRole(value_record.identity().layout()))?;
    let instance_layout = instance_record
        .instance_handle()
        .ok_or_else(|| ExactDescriptorError::LayoutRole(instance_record.identity().layout()))?;
    if value_layout.identity().exact_record() != instance_layout.identity().exact_record() {
        return Err(ExactDescriptorError::LayoutExact(exact));
    }
    if semantic.instance_layout() != instance_layout.identity().layout() {
        return Err(ExactDescriptorError::InstanceLayout(
            semantic.instance_layout(),
        ));
    }
    if semantic.instance_scan() != instance_record.scan() {
        return Err(ExactDescriptorError::InstanceScan(semantic.instance_scan()));
    }
    if semantic.instance_shape() != instance_layout.shape() {
        return Err(ExactDescriptorError::InstanceShape(exact));
    }
    validate_inline_scan(semantic, instance_record, &instance_layout, layouts)?;

    let mut tables = BTreeSet::new();
    let mut interfaces = BTreeSet::new();
    let itable_count = semantic.itables().len() as u64;
    let auxiliary_slots = itable_count
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or(ExactDescriptorError::CountOverflow)?;
    let comparison_work = itable_count
        .checked_mul(u64::from(itable_count.max(1).ilog2()) + 1)
        .and_then(|count| count.checked_mul(2))
        .ok_or(ExactDescriptorError::CountOverflow)?;
    meter.charge_collection_slots(auxiliary_slots, &path)?;
    meter.charge_work(comparison_work, &path)?;
    let vtable = semantic.vtable().table();
    validate_dispatch_key(foundation, vtable, exact, None, meter)?;
    tables.insert(vtable);
    let mut ancestry_interfaces = Vec::new();
    let mut itables = Vec::new();
    meter.try_reserve_collection_slots(
        &mut ancestry_interfaces,
        semantic.itables().len(),
        &path,
    )?;
    meter.try_reserve_collection_slots(&mut itables, semantic.itables().len(), &path)?;
    for itable in semantic.itables() {
        let interface = itable.interface();
        if !interfaces.insert(interface.exact_type()) {
            return Err(ExactDescriptorError::DuplicateInterface(
                interface.exact_type(),
            ));
        }
        if !tables.insert(itable.table()) {
            return Err(ExactDescriptorError::DuplicateDispatchTable(itable.table()));
        }
        validate_dispatch_key(
            foundation,
            itable.table(),
            exact,
            Some(interface.exact_type()),
            meter,
        )?;
        ancestry_interfaces.push(interface);
        itables.push(ExactDescriptorItableV1 {
            interface,
            table: itable.table(),
        });
    }

    let diagnostic_name =
        CanonicalExactTypeDiagnosticName::from_validated_graph_metered(exact, diagnostics, meter)?;
    if diagnostic_name.as_str() != semantic.diagnostic_name() {
        return Err(ExactDescriptorError::DiagnosticName(exact));
    }
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        foundation,
        meter,
    )?;
    let definition =
        StrongShapeDefinitionV1::from_artifact(exact, physical.definition(), physical.symbol());
    let registration_physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeRegistration(exact),
        foundation,
        meter,
    )?;
    if registration.semantic_id() != exact {
        return Err(ExactDescriptorError::RegistrationExact(exact));
    }
    if registration.definition_plan() != registration_physical.definition() {
        return Err(ExactDescriptorError::RegistrationDefinition(exact));
    }
    if registration.symbol() != registration_physical.symbol() {
        return Err(ExactDescriptorError::RegistrationSymbol(exact));
    }
    let expected_fingerprint = scoop_identity::DigestNodeId::from_key(
        &scoop_identity::DigestNodeKey::strong_registration(registration.definition_plan()),
    )
    .map_err(ExactDescriptorError::RegistrationFingerprintHash)?;
    if registration.fingerprint_node() != expected_fingerprint {
        return Err(ExactDescriptorError::RegistrationFingerprint(exact));
    }
    Ok(ExactDescriptorExportV1::from_parts(DescriptorBodyPartsV1 {
        exact: value_layout.identity().exact_record().clone(),
        value_layout,
        instance_layout,
        shape: semantic.instance_shape().clone(),
        object_scan: semantic.instance_shape().object_scan().clone(),
        ancestry: ExactDescriptorAncestryV1 {
            parent: semantic.parent(),
            interfaces: ancestry_interfaces,
        },
        dispatch: ExactDescriptorDispatchV1 { vtable, itables },
        diagnostic_name,
        physical,
        definition,
        registration,
    }))
}

pub(crate) fn validate_inline_scan(
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    instance_record: &crate::ExactLayoutExportV1,
    instance: &crate::ExactInstanceLayoutV1,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> Result<(), ExactDescriptorError> {
    let expected = match instance.representation().kind() {
        InstanceRepresentationKindV1::BoxedPayload(value) => {
            inline_scan(value.layout(), instance.shape(), layouts)?
        }
        InstanceRepresentationKindV1::InlineArray { .. } => instance
            .shape()
            .inline_scan()
            .contains_reference()
            .then_some(instance_record.scan()),
        InstanceRepresentationKindV1::ClassObject(_)
        | InstanceRepresentationKindV1::InlineBytes
        | InstanceRepresentationKindV1::AbstractReference => None,
    };
    let actual = match semantic.inline_scan() {
        TypeDescriptorInlineScanV1::Null => None,
        TypeDescriptorInlineScanV1::Defined(scan) => Some(scan),
    };
    if expected == actual {
        Ok(())
    } else {
        Err(ExactDescriptorError::InlineScan(semantic.exact_type()))
    }
}

fn inline_scan(
    layout: scoop_identity::PersistentLayoutId,
    shape: &crate::TypeInstanceShapeV1,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> Result<Option<scoop_identity::PersistentScanId>, ExactDescriptorError> {
    if !shape.inline_scan().contains_reference() {
        return Ok(None);
    }
    layouts
        .get(layout)
        .map(|record| Some(record.scan()))
        .ok_or(ExactDescriptorError::MissingInlineLayout(layout))
}

fn validate_dispatch_key(
    foundation: &OdrFreeLirFoundation,
    table: PersistentDispatchTableId,
    owner: PersistentExactTypeId,
    interface: Option<PersistentExactTypeId>,
    meter: &mut BudgetMeter,
) -> Result<(), ExactDescriptorError> {
    meter.charge_work(foundation.dispatch_tables().len() as u64, &WirePath::root())?;
    let expected = match interface {
        None => scoop_identity::DispatchTableKey::vtable(owner),
        Some(interface) => scoop_identity::DispatchTableKey::itable(owner, interface),
    };
    foundation
        .dispatch_tables()
        .iter()
        .any(|record| record.id() == table && record.key() == &expected)
        .then_some(())
        .ok_or(ExactDescriptorError::DispatchTable(table))
}

#[cfg(test)]
pub(super) fn replay_test_parts(
    target: crate::LirTargetProfile,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactDescriptorExportV1, ExactDescriptorError> {
    replay_parts(
        target,
        layouts,
        semantic,
        registration,
        diagnostics,
        foundation,
        meter,
    )
}
