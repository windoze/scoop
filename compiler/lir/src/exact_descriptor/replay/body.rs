use super::*;

pub(crate) fn replay_parts(
    target: crate::LirTargetProfile,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    foundation: &ConeLirFoundation,
) -> Result<ExactDescriptorExportV1, ExactDescriptorError> {
    let path = WirePath::root();

    if layouts.provider() != foundation.producer() {
        return Err(ExactDescriptorError::Provider);
    }
    if layouts.target() != target {
        return Err(ExactDescriptorError::Target);
    }
    let exact = semantic.exact_type();

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

    if semantic
        .itables()
        .windows(2)
        .any(|pair| pair[0].interface().exact_type() >= pair[1].interface().exact_type())
    {
        return Err(ExactDescriptorError::NonCanonicalInterfaces(exact));
    }

    let vtable = semantic.vtable().table();
    validate_dispatch_key(foundation, vtable, exact, None)?;
    tables.insert(vtable);
    let mut ancestry_interfaces = Vec::new();
    let mut itables = Vec::new();
    scoop_wire::allocation::try_reserve(&mut ancestry_interfaces, semantic.itables().len(), &path)?;
    scoop_wire::allocation::try_reserve(&mut itables, semantic.itables().len(), &path)?;
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
        )?;
        ancestry_interfaces.push(interface);
        itables.push(ExactDescriptorItableV1 {
            interface,
            table: itable.table(),
        });
    }

    let diagnostic_name =
        CanonicalExactTypeDiagnosticName::from_validated_graph(exact, diagnostics)?;
    if diagnostic_name.as_str() != semantic.diagnostic_name() {
        return Err(ExactDescriptorError::DiagnosticName(exact));
    }
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        foundation,
    )?;
    let definition =
        StrongShapeDefinitionV1::from_artifact(exact, physical.definition(), physical.symbol());
    let registration_physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeRegistration(exact),
        foundation,
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

fn validate_dispatch_key(
    foundation: &ConeLirFoundation,
    table: PersistentDispatchTableId,
    owner: PersistentExactTypeId,
    interface: Option<PersistentExactTypeId>,
) -> Result<(), ExactDescriptorError> {
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
