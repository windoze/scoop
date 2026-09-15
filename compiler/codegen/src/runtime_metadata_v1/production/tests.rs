use inkwell::context::Context;
use inkwell::targets::TargetData;
use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, DigestNodeV1, EntryProductionSourceV1, LirTargetProfile,
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1,
    StrongProductionSectionV1, StrongRegistrationProductionSurfaceV1,
};

use super::emit_strong_runtime_metadata_v1;

#[test]
fn emits_the_closed_runtime_surface_and_exact_patch_sidecar() {
    let (production, definition, primary_atom, patch_intent, symbol) = production();
    let context = Context::create();
    let llvm = context.create_module("strong-runtime-metadata");
    let target_data =
        TargetData::create(LirTargetProfile::DARWIN_AARCH64.canonical_llvm_data_layout());
    let producer = production.image_plan().cone().identity();
    let bounds_message = crate::emission::emit_cone_trap_message(
        &context,
        &llvm,
        &target_data,
        production.canonical_definitions(),
        producer,
        (
            ConeImageSupportRole::ArrayBoundsMessage,
            b"array index out of bounds",
        ),
        true,
    )
    .unwrap();
    let array_size_message = crate::emission::emit_cone_trap_message(
        &context,
        &llvm,
        &target_data,
        production.canonical_definitions(),
        producer,
        (
            ConeImageSupportRole::ArraySizeOverflowMessage,
            b"array size overflow",
        ),
        true,
    )
    .unwrap();

    let emitted = emit_strong_runtime_metadata_v1(
        &context,
        &llvm,
        &target_data,
        &production,
        bounds_message,
        array_size_message,
    )
    .unwrap();

    assert_eq!(
        emitted.producer(),
        production.image_plan().cone().identity()
    );
    assert_eq!(emitted.patch_locations().len(), 1);
    let patch = emitted.patch_locations()[0];
    assert_eq!(patch.intent(), patch_intent);
    assert_eq!(patch.definition(), definition);
    assert_eq!(patch.atom(), primary_atom);
    assert_eq!(patch.owner(), symbol);
    assert_eq!(patch.offset_within_owner(), 96);
    assert_eq!(patch.width_bytes(), 32);
    assert!(
        llvm.get_global(symbol.symbol().as_str())
            .unwrap()
            .get_initializer()
            .is_some()
    );
    let llvm_ir = llvm.print_to_string().to_string();
    let boundaries = production
        .canonical_definitions()
        .plans()
        .iter()
        .flat_map(|plan| plan.atom_boundaries());
    let message_atoms = [
        production
            .image_plan()
            .support_atoms()
            .array_bounds_message(),
        production
            .image_plan()
            .support_atoms()
            .array_size_overflow_message(),
    ];
    let mut expected_aliases = 0;
    for boundary in boundaries {
        for request in [boundary.start(), boundary.end()] {
            if message_atoms.contains(&boundary.atom()) && request == boundary.start() {
                assert!(
                    llvm.get_global(request.symbol().as_str()).is_some(),
                    "missing strong trap-message definition `{}`",
                    request.symbol()
                );
                continue;
            }
            expected_aliases += 1;
            assert!(
                llvm_ir.lines().any(|line| {
                    line.contains(request.symbol().as_str()) && line.contains(" = alias ")
                }),
                "missing strong atom boundary alias `{}` in:\n{llvm_ir}",
                request.symbol()
            );
        }
    }
    assert_eq!(llvm_ir.matches(" = alias ").count(), expected_aliases);
    llvm.verify().unwrap();
}

fn production() -> (
    StrongProductionSectionV1,
    ObjectDefinitionPlanId,
    ObjectDefinitionAtomId,
    scoop_identity::DigestPatchIntentId,
    PersistentSymbolRequest,
) {
    let coordinate = ConeCoordinate::reserved_single_file();
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
    let atoms = image_atoms(definition_id);
    let primary_atom = atoms[0].id();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);
    let image_id = DigestNodeId::from_key(&image_key).unwrap();
    let patch_key = DigestPatchIntentKey::new(
        image_id,
        definition_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let patch_intent = scoop_identity::DigestPatchIntentId::from_key(&patch_key).unwrap();
    let image = DigestNodeV1::new(image_key, Vec::new(), vec![patch_key]).unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();
    let registrations = StrongRegistrationProductionSurfaceV1::empty(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        &digests,
    )
    .unwrap();
    let external = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let production = StrongProductionSectionV1::new(
        coordinate,
        &foundation,
        external,
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
        scoop_lir::CoreLirBridgeBranchV1::NotCore,
    )
    .unwrap();
    (
        production,
        definition_id,
        primary_atom,
        patch_intent,
        symbol,
    )
}

fn image_atoms(
    definition: ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let primary = ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    );
    let supports = [
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
    ];
    std::iter::once(primary)
        .chain(supports.map(|(role, support)| {
            ObjectDefinitionAtomKey::new(
                definition,
                role,
                DefinitionAtomSubkey::ConeImageSupport(support),
            )
        }))
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
}
