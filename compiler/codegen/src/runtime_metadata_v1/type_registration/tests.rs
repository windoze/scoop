use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::{GlobalValue, StructValue, UnnamedAddress};
use scoop_identity::ConeIdentity;

use super::{
    DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET, DIGEST_SIZE, LAYOUT_FINGERPRINT_OFFSET,
    REGISTRATION_DEFINITION_FINGERPRINT_OFFSET, TYPE_REGISTRATION_DESCRIPTOR_MAGIC,
    TYPE_REGISTRATION_DESCRIPTOR_SIZE, emit_strong_type_registrations_v1,
};
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

mod support;
use support::type_plan;

#[test]
fn emits_closed_strong_record_descriptor_and_three_zero_patch_sites() {
    let plan = type_plan(1);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("type-registration");
    let type_descriptor = declare_type_descriptor(&context, &llvm, expected.descriptor_symbol());

    let emitted = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap();

    assert_eq!(emitted.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(emitted.registrations().len(), 1);
    let registration = emitted.registrations()[0];
    assert_eq!(registration.exact_type(), expected.exact_type());
    let descriptor = registration.descriptor();
    assert_eq!(descriptor.get_linkage(), Linkage::External);
    assert!(descriptor.is_constant());

    assert_patch(
        registration.registration_definition_patch(),
        expected.registration_definition_patch(),
        expected.definition_plan(),
        expected.primary_atom(),
        descriptor,
        REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
    );
    assert_patch(
        registration.descriptor_definition_patch(),
        expected.descriptor_definition_patch(),
        expected.definition_plan(),
        expected.primary_atom(),
        descriptor,
        DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET,
    );
    assert_patch(
        registration.layout_fingerprint_patch(),
        expected.layout_fingerprint_patch(),
        expected.definition_plan(),
        expected.primary_atom(),
        descriptor,
        LAYOUT_FINGERPRINT_OFFSET,
    );

    let initializer = descriptor.get_initializer().unwrap().into_struct_value();
    let prefix = initializer
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(prefix, 0), TYPE_REGISTRATION_DESCRIPTOR_MAGIC);
    assert_eq!(constant_u64(prefix, 1), 1);
    assert_eq!(constant_u64(prefix, 2), TYPE_REGISTRATION_DESCRIPTOR_SIZE);
    let identity = initializer
        .get_field_at_index(1)
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(identity, 0), 1);
    assert_eq!(constant_u64(identity, 1), 0);
    assert_eq!(
        identity.get_field_at_index(2).unwrap().into_struct_value(),
        super::super::registration_identity::digest_value(
            &context,
            RuntimeMetadataV1Types::new(&context).digest,
            expected.exact_type().as_array(),
        )
    );
    for index in 3..=5 {
        assert!(
            identity
                .get_field_at_index(index)
                .unwrap()
                .into_struct_value()
                .is_null()
        );
    }
    assert_eq!(constant_u64(initializer, 2), expected.runtime_type().get());
    assert_eq!(constant_u64(initializer, 3), 0);
    assert_eq!(
        initializer
            .get_field_at_index(4)
            .unwrap()
            .into_pointer_value(),
        type_descriptor.as_pointer_value()
    );
    for index in 5..=6 {
        assert!(
            initializer
                .get_field_at_index(index)
                .unwrap()
                .into_struct_value()
                .is_null()
        );
    }
    llvm.verify().unwrap();
}

#[test]
fn completes_a_matching_image_declaration_then_rejects_redefinition() {
    let plan = type_plan(1);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("type-registration-declaration");
    declare_type_descriptor(&context, &llvm, expected.descriptor_symbol());
    let types = RuntimeMetadataV1Types::new(&context);
    let declaration = llvm.add_global(
        types.type_registration_descriptor,
        None,
        expected.symbol().symbol().as_str(),
    );
    declaration.set_linkage(Linkage::External);

    let emitted = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap();
    assert_eq!(
        emitted.registrations()[0].descriptor().get_name(),
        declaration.get_name()
    );
    assert!(declaration.get_initializer().is_some());

    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("already defined"), "{error}");
}

#[test]
fn rejects_missing_incompatible_or_non_address_significant_type_descriptors() {
    let plan = type_plan(1);
    let expected = &plan.registrations()[0];
    let context = Context::create();

    let llvm = context.create_module("missing-type-descriptor");
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("is not declared"), "{error}");

    let llvm = context.create_module("incompatible-type-descriptor");
    let i64 = context.i64_type();
    let ptr = context.ptr_type(AddressSpace::default());
    let incompatible = context.struct_type(
        &[
            i64.into(),
            i64.into(),
            i64.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            i64.into(),
            ptr.into(),
        ],
        false,
    );
    let descriptor = llvm.add_global(
        incompatible,
        None,
        expected.descriptor_symbol().symbol().as_str(),
    );
    descriptor.set_linkage(Linkage::External);
    descriptor.set_constant(true);
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(
        error.0.contains("incompatible M23 LLVM declaration"),
        "{error}"
    );

    let llvm = context.create_module("unnamed-type-descriptor");
    let descriptor = declare_type_descriptor(&context, &llvm, expected.descriptor_symbol());
    descriptor.set_unnamed_address(UnnamedAddress::Global);
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(
        error.0.contains("incompatible M23 LLVM declaration"),
        "{error}"
    );
}

#[test]
fn rejects_mutable_descriptor_definitions_and_registration_collisions() {
    let plan = type_plan(1);
    let expected = &plan.registrations()[0];
    let context = Context::create();

    let llvm = context.create_module("mutable-type-descriptor");
    let descriptor = declare_type_descriptor(&context, &llvm, expected.descriptor_symbol());
    descriptor.set_initializer(
        &RuntimeMetadataV1Types::new(&context)
            .type_descriptor
            .const_zero(),
    );
    descriptor.set_constant(false);
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(
        error.0.contains("incompatible M23 LLVM declaration"),
        "{error}"
    );

    let llvm = context.create_module("type-registration-collision");
    declare_type_descriptor(&context, &llvm, expected.descriptor_symbol());
    let collision = llvm.add_global(context.i8_type(), None, expected.symbol().symbol().as_str());
    collision.set_linkage(Linkage::External);
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");
}

#[test]
fn validates_the_complete_set_before_emitting_any_registration() {
    let plan = type_plan(2);
    let context = Context::create();
    let llvm = context.create_module("atomic-type-registration-validation");
    declare_type_descriptor(&context, &llvm, plan.registrations()[0].descriptor_symbol());

    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("is not declared"), "{error}");
    for registration in plan.registrations() {
        assert!(
            llvm.get_global(registration.symbol().symbol().as_str())
                .is_none()
        );
    }
}

fn declare_type_descriptor<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    request: scoop_lir::PersistentSymbolRequest,
) -> GlobalValue<'ctx> {
    let descriptor = llvm.add_global(
        RuntimeMetadataV1Types::new(context).type_descriptor,
        None,
        request.symbol().as_str(),
    );
    descriptor.set_linkage(Linkage::External);
    descriptor.set_constant(true);
    descriptor
}

fn assert_patch(
    patch: super::TypeRegistrationPatchSiteV1<'_>,
    intent: scoop_lir::DigestPatchIntentId,
    definition: scoop_lir::ObjectDefinitionPlanId,
    atom: scoop_lir::ObjectDefinitionAtomId,
    owner: GlobalValue<'_>,
    byte_offset: u64,
) {
    assert_eq!(patch.intent(), intent);
    assert_eq!(patch.definition(), definition);
    assert_eq!(patch.atom(), atom);
    assert_eq!(patch.byte_offset(), byte_offset);
    assert_eq!(patch.byte_size(), DIGEST_SIZE);
    assert_eq!(patch.owner().get_name(), owner.get_name());
}

fn constant_u64(value: StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}
