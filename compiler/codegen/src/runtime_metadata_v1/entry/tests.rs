use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::types::AnyType;
use inkwell::values::AnyValue;
use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactCallableSignature, ExactTypeKey,
    ExecutableEntryIdentity, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PackagePath, PersistentCallableBodyId, PersistentExactTypeId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite, StaticStorageKey,
    StrongDefinitionEntity, StrongDefinitionRole,
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
    let source = executable_source_entry();
    let generated = CborIdentityRecord::from_key(scoop_identity::GeneratedCallableKey::TestEntry {
        cone: source.root_cone(),
    })
    .unwrap();
    let generated =
        ExecutableEntryIdentity::for_test(&generated, source.source_signature().clone()).unwrap();
    for entry in [source, generated] {
        assert_entry_emission(plan_for_entry(entry));
    }
}

fn assert_entry_emission(plan: EntryProductionPlanV1) {
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
    let pointer = context.ptr_type(inkwell::AddressSpace::default());
    assert_eq!(
        gateway.get_type(),
        context.i32_type().fn_type(
            &[context.i32_type().into(), pointer.into(), pointer.into()],
            false
        )
    );
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

mod support;
use support::*;
