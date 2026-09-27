use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::types::AnyType;
use inkwell::values::AnyValue;
use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
    ExecutableSourceEntryIdentity, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PackagePath, PersistentCallableBodyId, PersistentExactTypeId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite, StaticStorageKey,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestNodeV1,
    EntryProductionPlanV1, EntryProductionSourceV1, RegistrationIdentitySurfaceV1,
};

use super::{
    DIGEST_SIZE, EmittedEntryProductionV1, GATEWAY_DEFINITION_FINGERPRINT_OFFSET,
    ROOT_ENTRY_DESCRIPTOR_SIZE, SOURCE_SIGNATURE_FINGERPRINT_OFFSET, emit_entry_production_v1,
};
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

#[test]
fn library_entry_emission_has_no_root_surface() {
    let context = Context::create();
    let llvm = context.create_module("library-entry");

    let emitted =
        emit_entry_production_v1(&context, &llvm, &EntryProductionPlanV1::Library).unwrap();

    assert!(matches!(emitted, EmittedEntryProductionV1::Library));
    assert!(llvm.get_first_global().is_none());
    assert!(llvm.get_first_function().is_none());
    llvm.verify().unwrap();
}

#[test]
fn executable_entry_emission_preserves_typed_references_and_zero_patches() {
    let plan = executable_plan();
    let EntryProductionPlanV1::Executable(entry) = &plan else {
        panic!("fixture is executable")
    };
    let context = Context::create();
    let llvm = context.create_module("executable-entry");

    let emitted = emit_entry_production_v1(&context, &llvm, &plan).unwrap();
    let EmittedEntryProductionV1::Executable(emitted) = emitted else {
        panic!("executable plan emits an executable root")
    };

    let descriptor = emitted.descriptor();
    assert_eq!(descriptor.get_linkage(), Linkage::External);
    assert_eq!(descriptor.get_visibility(), GlobalVisibility::Default);
    assert!(descriptor.is_constant());
    assert_eq!(
        emitted.source_signature_patch().intent(),
        entry.source_signature_patch()
    );
    assert_eq!(
        emitted.gateway_definition_patch().intent(),
        entry.gateway_definition_patch()
    );
    for patch in [
        emitted.source_signature_patch(),
        emitted.gateway_definition_patch(),
    ] {
        assert_eq!(patch.definition(), entry.root_descriptor_definition());
        assert_eq!(patch.owner(), descriptor);
        assert_eq!(patch.byte_size(), DIGEST_SIZE);
    }
    assert_eq!(
        emitted.source_signature_patch().byte_offset(),
        SOURCE_SIGNATURE_FINGERPRINT_OFFSET
    );
    assert_eq!(
        emitted.gateway_definition_patch().byte_offset(),
        GATEWAY_DEFINITION_FINGERPRINT_OFFSET
    );

    let types = RuntimeMetadataV1Types::new(&context);
    let failure_root_symbol = entry.failure_root_registration_symbol().unwrap().symbol();
    let failure_root = llvm.get_global(failure_root_symbol.as_str()).unwrap();
    assert_eq!(
        failure_root.get_value_type(),
        types.static_storage_descriptor.as_any_type_enum()
    );
    assert_eq!(failure_root.get_linkage(), Linkage::External);
    let gateway_symbol = entry.gateway_symbol().unwrap().symbol();
    let gateway = llvm.get_function(gateway_symbol.as_str()).unwrap();
    assert_eq!(gateway.get_type(), context.i32_type().fn_type(&[], false));
    assert_eq!(gateway.get_linkage(), Linkage::External);

    let descriptor_ir = descriptor.print_to_string().to_string();
    assert!(descriptor_ir.contains("constant"), "{descriptor_ir}");
    assert!(
        descriptor_ir.contains(&ROOT_ENTRY_DESCRIPTOR_SIZE.to_string()),
        "{descriptor_ir}"
    );
    assert_eq!(descriptor_ir.matches("zeroinitializer").count(), 2);
    assert!(
        descriptor_ir.contains(failure_root_symbol.as_str()),
        "{descriptor_ir}"
    );
    assert!(
        descriptor_ir.contains(gateway_symbol.as_str()),
        "{descriptor_ir}"
    );
    llvm.verify().unwrap();
}

#[test]
fn executable_entry_emission_rejects_a_duplicate_descriptor() {
    let plan = executable_plan();
    let context = Context::create();
    let llvm = context.create_module("duplicate-entry");

    emit_entry_production_v1(&context, &llvm, &plan).unwrap();
    let error = emit_entry_production_v1(&context, &llvm, &plan).unwrap_err();

    assert!(error.0.contains("already declared"), "{error}");
}

#[test]
fn executable_entry_emission_rejects_an_incompatible_gateway() {
    let plan = executable_plan();
    let EntryProductionPlanV1::Executable(entry) = &plan else {
        panic!("fixture is executable")
    };
    let context = Context::create();
    let llvm = context.create_module("bad-gateway");
    let symbol = entry.gateway_symbol().unwrap().symbol();
    llvm.add_function(
        symbol.as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );

    let error = emit_entry_production_v1(&context, &llvm, &plan).unwrap_err();

    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");
}

fn executable_plan() -> EntryProductionPlanV1 {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = executable_source_entry();
    let declaration = source.declaration();
    let main = source.main();
    let gateway =
        PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(producer, main)).unwrap();
    let failure_root = PersistentStaticStorageId::from_key(
        &StaticStorageKey::root_entry_failure_root(producer, main),
    )
    .unwrap();
    let main_body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(declaration),
    ))
    .unwrap();
    let gateway_body =
        RuntimeIdentityRecord::from_key(&CallableBodyKey::root_gateway(producer, main)).unwrap();
    let failure_storage =
        CborIdentityRecord::from_key(StaticStorageKey::root_entry_failure_root(producer, main))
            .unwrap();

    let definitions = [
        (
            StrongDefinitionEntity::callable_body(main.body()),
            StrongDefinitionRole::CallableBody,
        ),
        (
            StrongDefinitionEntity::callable_body(main.body()),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableBody,
        ),
        (
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::static_storage(failure_root),
            StrongDefinitionRole::StaticStorage,
        ),
        (
            StrongDefinitionEntity::static_storage(failure_root),
            StrongDefinitionRole::RootRegistration,
        ),
        (
            StrongDefinitionEntity::root_entry(producer),
            StrongDefinitionRole::RootEntryDescriptor,
        ),
    ];
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    for (entity, role) in definitions {
        let plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
        )
        .unwrap();
        atoms.push(
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                plan.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
        );
        plans.push(plan);
    }

    let symbols = [
        PersistentSymbolKey::CallableBody(main.body()),
        PersistentSymbolKey::CallableRegistration(main.body()),
        PersistentSymbolKey::CallableBody(gateway),
        PersistentSymbolKey::CallableRegistration(gateway),
        PersistentSymbolKey::StaticStorage(failure_root),
        PersistentSymbolKey::RootRegistration(failure_root),
        PersistentSymbolKey::RootEntryDescriptor(producer),
    ]
    .into_iter()
    .map(|key| PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap())
    .collect();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![main_body, gateway_body])
        .unwrap();
    canonical
        .set_static_storages(vec![failure_storage])
        .unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();

    let gateway_definition = definition_plan(
        producer,
        StrongDefinitionEntity::callable_body(gateway),
        StrongDefinitionRole::CallableBody,
    );
    let gateway_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        gateway_definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
    .id();
    let source_key = DigestNodeKey::source_signature(main.body());
    let source_id = DigestNodeId::from_key(&source_key).unwrap();
    let source_node = DigestNodeV1::new(
        source_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            source_id,
            definition_plan(
                producer,
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
            ),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::SourceSignature,
        )],
    )
    .unwrap();
    let gateway_key = DigestNodeKey::object_definition(gateway_atom);
    let gateway_id = DigestNodeId::from_key(&gateway_key).unwrap();
    let gateway_definition_node = DigestNodeV1::new(
        gateway_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            gateway_id,
            definition_plan(
                producer,
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
            ),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::GatewayDefinition,
        )],
    )
    .unwrap();
    let mut nodes = vec![source_node, gateway_definition_node];
    for (entity, role) in [
        (
            StrongDefinitionEntity::callable_body(main.body()),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::static_storage(failure_root),
            StrongDefinitionRole::RootRegistration,
        ),
    ] {
        let registration = definition_plan(producer, entity, role);
        nodes.push(
            DigestNodeV1::new(
                DigestNodeKey::strong_registration(registration),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        );
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(producer),
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
    );
    let digests = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let registrations =
        RegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    EntryProductionPlanV1::new(
        EntryProductionSourceV1::executable(source),
        &foundation,
        &registrations,
        &digests,
    )
    .unwrap()
}

fn definition_plan(
    producer: ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> scoop_identity::ObjectDefinitionPlanId {
    scoop_identity::ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
    )
    .unwrap()
}

fn executable_source_entry() -> ExecutableSourceEntryIdentity {
    let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("main").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    ExecutableSourceEntryIdentity::try_new(
        &declaration,
        ExactOrdinaryNoArgUnitSignature::new(unit_exact_type()),
    )
    .unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap()
}
