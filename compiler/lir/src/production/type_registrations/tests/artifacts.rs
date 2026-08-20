//! Canonical fixture identities and definition atoms.

use super::*;

pub(super) fn type_artifacts(seed: u8) -> TypeArtifacts {
    let exact_type = exact_type(&format!("Type{seed}"));
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type,
        RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::ManagedObject)).unwrap();
    let vtable = CborIdentityRecord::from_key(DispatchTableKey::vtable(exact_type)).unwrap();
    let descriptor_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    );
    let descriptor_primary = primary(&descriptor_definition);
    let descriptor_diagnostic = descriptor_diagnostic(&descriptor_definition, exact_type);
    let layout_definition = definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let layout_primary = primary(&layout_definition);
    let registration_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    );
    let registration_primary = primary(&registration_definition);
    TypeArtifacts {
        exact_type,
        layout,
        scan,
        vtable,
        descriptor_definition,
        descriptor_primary,
        descriptor_diagnostic,
        layout_definition,
        layout_primary,
        registration_definition,
        registration_primary,
    }
}

pub(super) fn descriptor_diagnostic(
    definition: &CborIdentityRecord<
        scoop_identity::ObjectDefinitionPlanId,
        ObjectDefinitionPlanKey,
    >,
    exact_type: PersistentExactTypeId,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::ExactType(exact_type),
    ))
    .unwrap()
}

pub(super) fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

pub(super) fn definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap()
}

pub(super) fn primary(
    definition: &CborIdentityRecord<
        scoop_identity::ObjectDefinitionPlanId,
        ObjectDefinitionPlanKey,
    >,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}
