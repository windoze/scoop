use super::*;

pub(super) struct RegistrationArtifacts {
    pub(super) site: PersistentSafepointSiteId,
    pub(super) plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) symbol: PersistentSymbolRequest,
}

pub(super) struct CallableRegistrationArtifacts {
    pub(super) plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) symbol: PersistentSymbolRequest,
}

pub(super) struct TypeRegistrationArtifacts {
    pub(super) exact_type: PersistentExactTypeId,
    pub(super) layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    pub(super) descriptor_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) descriptor_primary:
        CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) descriptor_diagnostic:
        CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) descriptor_symbol: PersistentSymbolRequest,
    pub(super) layout_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) layout_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) layout_symbol: PersistentSymbolRequest,
    pub(super) registration_plan:
        CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) registration_primary:
        CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) registration_symbol: PersistentSymbolRequest,
}

pub(super) struct ImmortalRegistrationArtifacts {
    pub(super) object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
    pub(super) object_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) object_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) object_symbol: PersistentSymbolRequest,
    pub(super) registration_plan:
        CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) registration_primary:
        CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) registration_symbol: PersistentSymbolRequest,
}

pub(super) struct StaticStorageArtifacts {
    pub(super) storage:
        CborIdentityRecord<PersistentStaticStorageId, scoop_identity::StaticStorageKey>,
    pub(super) layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    pub(super) scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    pub(super) storage_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) storage_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) template_atom:
        Option<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    pub(super) relocation_atom:
        Option<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    pub(super) storage_symbol: PersistentSymbolRequest,
    pub(super) registration_plan:
        CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) registration_primary:
        CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) registration_symbol: PersistentSymbolRequest,
    pub(super) layout_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) layout_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) layout_symbol: PersistentSymbolRequest,
    pub(super) scan_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) scan_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    pub(super) scan_symbol: PersistentSymbolRequest,
}

pub(super) fn static_storage_artifacts(module: &Module) -> StaticStorageArtifacts {
    let (identity, layout, initial_state) = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            GlobalInit::Storage {
                identity,
                layout,
                initial_state,
                ..
            } => Some((identity, layout, initial_state)),
            GlobalInit::CString { .. }
            | GlobalInit::StringConst { .. }
            | GlobalInit::RawStorage { .. }
            | GlobalInit::ImportedStorage { .. } => None,
        })
        .unwrap();
    let storage = identity.identity_record().clone();
    let layout = layout.local().unwrap();
    let layout_record = layout.layout_record().clone();
    let scan = layout.scan_record().clone();
    let storage_plan = strong_definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::StaticStorage,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::RootRegistration,
    );
    let layout_plan = strong_definition(
        StrongDefinitionEntity::layout(layout_record.id()),
        StrongDefinitionRole::Layout,
    );
    let scan_plan = strong_definition(
        StrongDefinitionEntity::scan(scan.id()),
        StrongDefinitionRole::ScanProgram,
    );
    let template_atom = matches!(
        initial_state,
        LirStaticInitialState::EncodedStaticValue { .. }
    )
    .then(|| {
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            storage_plan.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::StaticStorage(storage.id()),
        ))
        .unwrap()
    });
    let relocation_atom = matches!(
        initial_state,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer { .. }
        }
    )
    .then(|| {
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            storage_plan.id(),
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::StaticStorage(storage.id()),
        ))
        .unwrap()
    });
    StaticStorageArtifacts {
        storage_symbol: identity.symbol_request(),
        registration_symbol: strong_symbol(PersistentSymbolKey::RootRegistration(storage.id())),
        layout_symbol: strong_symbol(PersistentSymbolKey::Layout(layout_record.id())),
        scan_symbol: strong_symbol(PersistentSymbolKey::ScanProgram(scan.id())),
        storage_primary: primary_atom(&storage_plan),
        template_atom,
        relocation_atom,
        registration_primary: primary_atom(&registration_plan),
        layout_primary: primary_atom(&layout_plan),
        scan_primary: primary_atom(&scan_plan),
        storage,
        layout: layout_record,
        scan,
        storage_plan,
        registration_plan,
        layout_plan,
        scan_plan,
    }
}

pub(super) fn immortal_registration_artifacts(
    object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
) -> ImmortalRegistrationArtifacts {
    let object_plan = strong_definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalObject,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalRegistration,
    );
    ImmortalRegistrationArtifacts {
        object_symbol: strong_symbol(PersistentSymbolKey::ImmortalObject(object.id())),
        registration_symbol: strong_symbol(PersistentSymbolKey::ImmortalRegistration(object.id())),
        object_primary: primary_atom(&object_plan),
        registration_primary: primary_atom(&registration_plan),
        object,
        object_plan,
        registration_plan,
    }
}

pub(super) fn type_registration_artifacts(
    exact_type: PersistentExactTypeId,
) -> TypeRegistrationArtifacts {
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type,
        RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let descriptor_plan = strong_definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    );
    let layout_plan = strong_definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    );
    let layout_symbol = strong_symbol(PersistentSymbolKey::Layout(layout.id()));
    TypeRegistrationArtifacts {
        exact_type,
        layout,
        descriptor_primary: primary_atom(&descriptor_plan),
        descriptor_diagnostic: CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor_plan.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::ExactType(exact_type),
        ))
        .unwrap(),
        descriptor_symbol: strong_symbol(PersistentSymbolKey::TypeDescriptor(exact_type)),
        layout_primary: primary_atom(&layout_plan),
        layout_symbol,
        registration_primary: primary_atom(&registration_plan),
        registration_symbol: strong_symbol(PersistentSymbolKey::TypeRegistration(exact_type)),
        descriptor_plan,
        layout_plan,
        registration_plan,
    }
}

pub(super) fn strong_definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap()
}

pub(super) fn primary_atom(
    plan: &CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

pub(super) fn strong_symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap()
}

pub(super) fn callable_registration_artifacts(
    body: scoop_identity::PersistentCallableBodyId,
) -> CallableRegistrationArtifacts {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableRegistration(body),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    CallableRegistrationArtifacts {
        plan,
        primary,
        symbol,
    }
}

pub(super) fn registration_artifacts(site: PersistentSafepointSiteId) -> RegistrationArtifacts {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::safepoint_site(site),
            StrongDefinitionRole::SafepointRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::SafepointRegistration(site),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    RegistrationArtifacts {
        site,
        plan,
        primary,
        symbol,
    }
}
