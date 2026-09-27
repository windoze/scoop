use super::*;

pub(super) fn build_registration<D: StrongDescriptorReference, C: Clone>(
    target: crate::LirTargetProfile,
    foundation: &ConeLirFoundation,
    semantic: &StrongTypeDescriptorSemanticPlan<D, C>,
    identity: &crate::RegistrationIdentityV1<PersistentExactTypeId>,
    digests: &DigestFinalizationPlanV1,
) -> Result<StrongTypeRegistrationPlan<D, C>, StrongTypeRegistrationPlanBuildError> {
    let exact_type = semantic.exact_type();
    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                exact_type,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let symbol = require_symbol(
        foundation,
        PersistentSymbolKey::TypeRegistration(exact_type),
    )?;

    let descriptor_definition = require_definition(
        foundation,
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    )?;
    let descriptor_primary_atom = require_primary_atom(foundation, descriptor_definition.id())?;
    let (diagnostic_atom, itable_directory) = require_descriptor_associated_atoms(
        foundation,
        descriptor_definition.id(),
        exact_type,
        !semantic.itables().is_empty(),
    )?;
    let descriptor_symbol =
        require_symbol(foundation, PersistentSymbolKey::TypeDescriptor(exact_type))?;

    let runtime_types = foundation
        .runtime_types()
        .iter()
        .filter(|mapping| mapping.exact_type() == exact_type)
        .map(|mapping| mapping.runtime_type())
        .collect::<Vec<_>>();
    let runtime_type = match runtime_types.as_slice() {
        [runtime_type] => *runtime_type,
        _ => {
            return Err(StrongTypeRegistrationPlanBuildError::RuntimeTypeSet {
                exact_type,
                actual: runtime_types,
            });
        }
    };

    let layout = semantic.instance_layout();
    let layout_record = foundation
        .layouts()
        .iter()
        .find(|record| record.id() == layout)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingLayout(layout))?;
    if layout_record.key().exact_type() != exact_type
        || layout_record.key().target_profile() != &target.wire_id()
        || layout_record.key().representation() != RepresentationRole::ManagedObject
    {
        return Err(
            StrongTypeRegistrationPlanBuildError::InstanceLayoutMismatch { exact_type, layout },
        );
    }
    let scan = semantic.instance_scan();
    let scan_record = foundation
        .scans()
        .iter()
        .find(|record| record.id() == scan)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingScan(scan))?;
    if scan_record.key().layout() != layout
        || scan_record.key().role() != scoop_identity::ScanRole::ManagedObject
    {
        return Err(StrongTypeRegistrationPlanBuildError::InstanceScanMismatch {
            exact_type,
            scan,
        });
    }
    let vtable = semantic.vtable().table();
    let expected_vtable = scoop_identity::DispatchTableKey::vtable(exact_type);
    if !foundation
        .dispatch_tables()
        .iter()
        .any(|record| record.id() == vtable && record.key() == &expected_vtable)
    {
        return Err(StrongTypeRegistrationPlanBuildError::VtableMismatch {
            exact_type,
            table: vtable,
        });
    }
    for itable in semantic.itables() {
        let expected_itable =
            scoop_identity::DispatchTableKey::itable(exact_type, itable.interface().exact_type());
        if !foundation
            .dispatch_tables()
            .iter()
            .any(|record| record.id() == itable.table() && record.key() == &expected_itable)
        {
            return Err(StrongTypeRegistrationPlanBuildError::ItableMismatch {
                exact_type,
                table: itable.table(),
            });
        }
    }
    let layout_definition = require_definition(
        foundation,
        StrongDefinitionEntity::layout(layout),
        StrongDefinitionRole::Layout,
    )?;
    let layout_primary_atom = require_primary_atom(foundation, layout_definition.id())?;
    let layout_symbol = require_symbol(foundation, PersistentSymbolKey::Layout(layout))?;
    let inline_scan = match semantic.inline_scan() {
        TypeDescriptorInlineScanV1::Null => StrongTypeDescriptorInlineScanPlanV1::Null,
        TypeDescriptorInlineScanV1::Defined(scan) => {
            let definition = require_definition(
                foundation,
                StrongDefinitionEntity::scan(scan),
                StrongDefinitionRole::ScanProgram,
            )?;
            StrongTypeDescriptorInlineScanPlanV1::Defined {
                scan,
                definition_plan: definition.id(),
            }
        }
    };

    let registration_object =
        require_digest_node(digests, DigestNodeKey::object_definition(primary_atom))?;
    if !registration_object.direct_inputs().is_empty() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationObjectInputs {
                node: registration_object.id(),
                actual: registration_object.direct_inputs().to_vec(),
            },
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationObjectPatches {
                node: registration_object.id(),
                actual: registration_object
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let descriptor_definition_node = require_digest_node(
        digests,
        DigestNodeKey::object_definition(descriptor_primary_atom),
    )?;
    let layout_fingerprint_node = require_digest_node(digests, DigestNodeKey::layout(layout))?;
    let registration_fingerprint = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationDigestMismatch {
                exact_type,
                expected: registration_fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }

    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(descriptor_definition_node),
        DigestInputRefV1::from_node(layout_fingerprint_node),
    ];
    expected_inputs.sort_unstable();
    if registration_fingerprint.direct_inputs() != expected_inputs {
        return Err(StrongTypeRegistrationPlanBuildError::DirectInputs {
            node: registration_fingerprint.id(),
            expected: expected_inputs,
            actual: registration_fingerprint.direct_inputs().to_vec(),
        });
    }

    let registration_definition_patch = require_only_patch(
        registration_fingerprint,
        DigestPatchIntentKey::new(
            registration_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;
    let descriptor_definition_patch = require_patch(
        descriptor_definition_node,
        DigestPatchIntentKey::new(
            descriptor_definition_node.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::DescriptorDefinition,
        ),
    )?;
    let layout_fingerprint_patch = require_patch(
        layout_fingerprint_node,
        DigestPatchIntentKey::new(
            layout_fingerprint_node.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        ),
    )?;

    Ok(StrongTypeRegistrationPlan {
        semantic: semantic.clone(),
        runtime_type,
        symbol,
        definition_plan: registration_definition.id(),
        primary_atom,
        descriptor_symbol,
        descriptor_definition_plan: descriptor_definition.id(),
        descriptor_primary_atom,
        diagnostic_atom,
        itable_directory,
        layout_symbol,
        layout_definition_plan: layout_definition.id(),
        layout_primary_atom,
        inline_scan,
        registration_object_node: registration_object.id(),
        descriptor_definition_node: descriptor_definition_node.id(),
        layout_fingerprint_node: layout_fingerprint_node.id(),
        registration_fingerprint_node: registration_fingerprint.id(),
        registration_definition_patch,
        descriptor_definition_patch,
        layout_fingerprint_patch,
    })
}
