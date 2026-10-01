use super::*;

pub(crate) fn strong_production() -> ConeProductionSectionV1 {
    strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]).1
}

pub(crate) fn strong_production_fixture(
    coordinate: ConeCoordinate,
    direct_dependencies: &[ConeIdentity],
) -> (CanonicalLirFoundation, ConeProductionSectionV1) {
    let producer = coordinate.identity().unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let definition_id = definition.id();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical
        .set_definition_atoms(image_atoms(definition_id))
        .unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = ConeLirFoundation::try_new(producer, canonical.clone()).unwrap();
    let image_key = DigestNodeKey::runtime_image(producer);
    let image_id = DigestNodeId::from_key(&image_key).unwrap();
    let patch = DigestPatchIntentKey::new(
        image_id,
        definition_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image = scoop_lir::DigestNodeV1::new(image_key, Vec::new(), vec![patch]).unwrap();
    let digests = DigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();

    let registrations = scoop_lir::StrongRegistrationProductionSurfaceV1::empty(
        selection().target(),
        &foundation,
        &digests,
    )
    .unwrap();
    let production = ConeProductionSectionV1::new(
        coordinate,
        direct_dependencies,
        &foundation,
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
        scoop_lir::CanonicalCallableLirDefinitionsV1::new(Vec::new(), &foundation).unwrap(),
        scoop_lir::CanonicalShapeLirDefinitionsV1::new(Vec::new(), &foundation).unwrap(),
    )
    .unwrap();
    (canonical, production)
}

pub(crate) fn image_atoms(
    plan: ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let mut keys = vec![ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    )];
    keys.extend(
        [
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateGroup,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateName,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateVersion,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Dependencies,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::StaticStorages,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::ImmortalObjects,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::InitializationUnits,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::TypeRegistrations,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Safepoints,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Callables,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArrayBoundsMessage,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArraySizeOverflowMessage,
            ),
        ]
        .map(|(role, support)| {
            ObjectDefinitionAtomKey::new(
                plan,
                role,
                DefinitionAtomSubkey::ConeImageSupport(support),
            )
        }),
    );
    keys.into_iter()
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
}

pub(in super::super) fn cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::new("test", "strong-link", "0.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

pub(super) fn digest_patch_intent() -> scoop_identity::DigestPatchIntentId {
    strong_production().digest_finalization_plan().nodes()[0].patch_intents()[0].id()
}

pub(super) fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

pub(in super::super) fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
            DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
    )
    .unwrap()
}
