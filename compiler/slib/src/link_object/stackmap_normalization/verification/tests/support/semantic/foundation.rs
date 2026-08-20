use super::*;

pub(super) fn foundation(
    module: &Module,
    body: &CallableBodyIdentity,
    safepoints: &[SafepointIdentity],
    corruption: Corruption,
) -> (
    ConeLirFoundation,
    Vec<RegistrationArtifacts>,
    CallableRegistrationArtifacts,
    TypeRegistrationArtifacts,
    ImmortalRegistrationArtifacts,
    StaticStorageArtifacts,
) {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let stackmap = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        if matches!(corruption, Corruption::WrongStackmapAtomRole) {
            DefinitionAtomRole::AddressTakenConstant
        } else {
            DefinitionAtomRole::Stackmap
        },
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let registrations = safepoints
        .iter()
        .map(|safepoint| registration_artifacts(safepoint.site_id()))
        .collect::<Vec<_>>();
    let callable_registration = callable_registration_artifacts(body.id());
    let type_registration = type_registration_artifacts(exact_type("String"));
    let immortal = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            GlobalInit::StringConst { identity, .. } => Some(identity.identity_record().clone()),
            GlobalInit::CString { .. }
            | GlobalInit::Storage { .. }
            | GlobalInit::RawStorage { .. }
            | GlobalInit::ImportedStorage { .. } => None,
        })
        .unwrap();
    let immortal_registration = immortal_registration_artifacts(immortal);
    let static_storage = static_storage_artifacts(module);
    let local_type_descriptor = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor)
        .find(|descriptor| descriptor.identity.exact_type() == type_registration.exact_type)
        .expect("the semantic fixture defines its local String descriptor");
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![body.identity_record().clone()])
        .unwrap();
    canonical
        .set_safepoint_sites(
            safepoints
                .iter()
                .map(|identity| identity.site_record().clone())
                .collect(),
        )
        .unwrap();
    canonical
        .set_safepoints(
            safepoints
                .iter()
                .map(SafepointMappingRecord::from_identity)
                .collect(),
        )
        .unwrap();
    canonical
        .set_layouts(vec![
            type_registration.layout.clone(),
            static_storage.layout.clone(),
        ])
        .unwrap();
    canonical
        .set_scans(vec![
            local_type_descriptor.instance_layout.scan_record().clone(),
            static_storage.scan.clone(),
        ])
        .unwrap();
    canonical
        .set_dispatch_tables(vec![local_type_descriptor.vtable.identity_record().clone()])
        .unwrap();
    canonical
        .set_static_storages(vec![static_storage.storage.clone()])
        .unwrap();
    canonical
        .set_immortal_objects(vec![immortal_registration.object.clone()])
        .unwrap();
    canonical
        .set_runtime_types(vec![
            RuntimeTypeMappingRecord::new(type_registration.exact_type).unwrap(),
        ])
        .unwrap();
    canonical
        .set_definition_plans(
            std::iter::once(definition)
                .chain(std::iter::once(callable_registration.plan.clone()))
                .chain([
                    type_registration.descriptor_plan.clone(),
                    type_registration.layout_plan.clone(),
                    type_registration.registration_plan.clone(),
                ])
                .chain([
                    immortal_registration.object_plan.clone(),
                    immortal_registration.registration_plan.clone(),
                ])
                .chain([
                    static_storage.storage_plan.clone(),
                    static_storage.registration_plan.clone(),
                    static_storage.layout_plan.clone(),
                    static_storage.scan_plan.clone(),
                ])
                .chain(
                    registrations
                        .iter()
                        .map(|registration| registration.plan.clone()),
                )
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            [primary, stackmap]
                .into_iter()
                .chain(std::iter::once(callable_registration.primary.clone()))
                .chain([
                    type_registration.descriptor_primary.clone(),
                    type_registration.descriptor_diagnostic.clone(),
                    type_registration.layout_primary.clone(),
                    type_registration.registration_primary.clone(),
                ])
                .chain([
                    immortal_registration.object_primary.clone(),
                    immortal_registration.registration_primary.clone(),
                ])
                .chain(
                    [
                        Some(static_storage.storage_primary.clone()),
                        static_storage.template_atom.clone(),
                        static_storage.relocation_atom.clone(),
                        Some(static_storage.registration_primary.clone()),
                        Some(static_storage.layout_primary.clone()),
                        Some(static_storage.scan_primary.clone()),
                    ]
                    .into_iter()
                    .flatten(),
                )
                .chain(
                    registrations
                        .iter()
                        .map(|registration| registration.primary.clone()),
                )
                .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            std::iter::once(body.symbol_request())
                .chain(std::iter::once(callable_registration.symbol))
                .chain([
                    type_registration.descriptor_symbol,
                    type_registration.layout_symbol,
                    type_registration.registration_symbol,
                ])
                .chain([
                    immortal_registration.object_symbol,
                    immortal_registration.registration_symbol,
                ])
                .chain([
                    static_storage.storage_symbol,
                    static_storage.registration_symbol,
                    static_storage.layout_symbol,
                    static_storage.scan_symbol,
                ])
                .chain(registrations.iter().map(|registration| registration.symbol))
                .collect(),
        )
        .unwrap(),
    );
    (
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        registrations,
        callable_registration,
        type_registration,
        immortal_registration,
        static_storage,
    )
}
