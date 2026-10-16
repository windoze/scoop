use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::AnyValue;
use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, CoreBuiltinNominal,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, ConeImagePlanV1, ConeLirFoundation, DigestFinalizationPlanV1,
    DigestInputRefV1, DigestNodeV1, RegistrationIdentitySurfaceV1,
};

use super::{DIGEST_SIZE, IMAGE_DESCRIPTOR_SIZE, emit_cone_image_v1};

#[test]
fn image_emission_preserves_the_closed_plan_and_zero_patch() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digests) = image_fixture(coordinate.clone(), Some(unit_exact_type()));
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate,
        &[scoop_identity::ConeIdentity::CORE],
        &foundation,
        &registrations,
        &digests,
    )
    .unwrap();
    let context = Context::create();
    let llvm = context.create_module("image");
    let (bounds_message, array_size_message) = trap_messages(&context, &llvm, &plan);

    let emitted = emit_cone_image_v1(
        &context,
        &llvm,
        &plan,
        &registrations,
        bounds_message,
        array_size_message,
    );

    assert_eq!(emitted.image().get_linkage(), Linkage::External);
    assert_eq!(emitted.image().get_visibility(), GlobalVisibility::Hidden);
    assert!(emitted.image().is_constant());
    assert_eq!(emitted.patch().intent(), plan.fingerprint_patch());
    assert_eq!(emitted.patch().definition(), plan.definition_plan());
    assert_eq!(emitted.primary_atom(), plan.primary_atom());
    assert_eq!(emitted.patch().atom(), plan.primary_atom());
    assert_eq!(
        emitted.support_atoms().dependencies().atom(),
        plan.support_atoms().dependencies()
    );
    assert_eq!(
        emitted.support_atoms().callables().atom(),
        plan.support_atoms().callables()
    );
    assert_eq!(
        emitted.support_atoms().array_bounds_message().atom(),
        plan.support_atoms().array_bounds_message()
    );
    assert_eq!(
        emitted.support_atoms().array_size_overflow_message().atom(),
        plan.support_atoms().array_size_overflow_message()
    );
    assert!(
        emitted
            .support_atoms()
            .dependencies()
            .global()
            .is_constant()
    );
    assert_eq!(emitted.patch().byte_offset(), 96);
    assert_eq!(emitted.patch().byte_size(), DIGEST_SIZE);
    let image_ir = emitted.image().print_to_string().to_string();
    assert!(image_ir.contains("hidden constant"), "{image_ir}");
    assert!(image_ir.contains("zeroinitializer"), "{image_ir}");
    assert!(
        image_ir.contains(&IMAGE_DESCRIPTOR_SIZE.to_string()),
        "{image_ir}"
    );
    assert!(
        llvm.get_global(&format!(
            "{}.metadata.dependencies",
            plan.symbol().symbol().as_str()
        ))
        .is_some()
    );
    let type_registration = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeRegistration(unit_exact_type()),
        LinkageClass::ConeStrong,
    )
    .unwrap()
    .symbol();
    assert!(llvm.get_global(type_registration.as_str()).is_some());
    llvm.verify().unwrap();
}

fn trap_messages<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: &ConeImagePlanV1,
) -> (
    inkwell::values::GlobalValue<'ctx>,
    inkwell::values::GlobalValue<'ctx>,
) {
    let define = |atom, bytes: &[u8]| {
        let request = PersistentSymbolRequest::new(
            PersistentSymbolKey::DefinitionBoundaryStart(atom),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        let initializer = context.const_string(bytes, true);
        let global = llvm.add_global(initializer.get_type(), None, request.symbol().as_str());
        global.set_linkage(Linkage::External);
        global.set_constant(true);
        global.set_initializer(&initializer);
        global
    };
    (
        define(
            plan.support_atoms().array_bounds_message(),
            b"array index out of bounds",
        ),
        define(
            plan.support_atoms().array_size_overflow_message(),
            b"array size overflow",
        ),
    )
}

fn image_fixture(
    coordinate: ConeCoordinate,
    registration: Option<PersistentExactTypeId>,
) -> (ConeLirFoundation, DigestFinalizationPlanV1) {
    let producer = coordinate.identity().unwrap();
    let image_plan_key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::cone_image(producer),
        StrongDefinitionRole::ImageDescriptor,
    )
    .unwrap();
    let image_plan = CborIdentityRecord::from_key(image_plan_key).unwrap();
    let image_plan_id = image_plan.id();
    let image_atoms = image_atoms(image_plan.id());
    let image_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();

    let mut plans = vec![image_plan];
    let mut nodes = Vec::new();
    if let Some(exact) = registration {
        let registration_plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
            )
            .unwrap(),
        )
        .unwrap();
        plans.push(registration_plan);
    }

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(image_atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![image_symbol]).unwrap());
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);
    let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
    let image_patch = DigestPatchIntentKey::new(
        image_node_id,
        image_plan_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let inputs = nodes.iter().map(DigestInputRefV1::from_node).collect();
    nodes.push(DigestNodeV1::new(image_key, inputs, vec![image_patch]).unwrap());
    let digest_plan = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    (foundation, digest_plan)
}

fn image_atoms(
    plan: scoop_identity::ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
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

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}
