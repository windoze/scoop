use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::UnnamedAddress;
use scoop_identity::ConeIdentity;

use super::{
    BODY_DEFINITION_FINGERPRINT_OFFSET, CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC,
    CALLABLE_REGISTRATION_DESCRIPTOR_SIZE, DIGEST_SIZE, emit_strong_callable_registrations_v1,
};
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

mod support;
use support::callable_plan;

#[test]
fn emits_closed_strong_record_entry_and_zero_body_patch_site() {
    let (plan, surface) = callable_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("callable-registration");
    let entry = llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );

    let emitted = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap();

    assert_eq!(emitted.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(emitted.registrations().len(), 1);
    let registration = emitted.registrations()[0];
    assert_eq!(registration.body(), expected.body());
    let descriptor = registration.descriptor();
    assert_eq!(descriptor.get_linkage(), Linkage::External);
    assert!(descriptor.is_constant());
    let body_patch = registration.body_definition_patch();
    assert_eq!(body_patch.intent(), expected.body_definition_patch());
    assert_eq!(body_patch.definition(), expected.definition_plan());
    assert_eq!(body_patch.atom(), expected.primary_atom());
    assert_eq!(body_patch.byte_offset(), BODY_DEFINITION_FINGERPRINT_OFFSET);
    assert_eq!(body_patch.byte_size(), DIGEST_SIZE);

    let initializer = descriptor.get_initializer().unwrap().into_struct_value();
    let prefix = initializer
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(
        constant_u64(prefix, 0),
        CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC
    );
    assert_eq!(constant_u64(prefix, 1), 6);
    assert_eq!(
        constant_u64(prefix, 2),
        CALLABLE_REGISTRATION_DESCRIPTOR_SIZE
    );
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
            types(&context).digest,
            expected.body().as_array()
        )
    );
    for index in 3..=4 {
        assert!(
            identity
                .get_field_at_index(index)
                .unwrap()
                .into_struct_value()
                .is_null()
        );
    }
    assert!(
        initializer
            .get_field_at_index(2)
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert_eq!(
        initializer
            .get_field_at_index(3)
            .unwrap()
            .into_pointer_value(),
        entry.as_global_value().as_pointer_value()
    );
    llvm.verify().unwrap();
}

#[test]
fn completes_a_matching_image_declaration_then_rejects_redefinition() {
    let (plan, surface) = callable_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("callable-registration-declaration");
    let types = RuntimeMetadataV1Types::new(&context);
    let symbol = expected.symbol().symbol();
    let declaration = llvm.add_global(
        types.callable_registration_descriptor,
        None,
        symbol.as_str(),
    );
    declaration.set_linkage(Linkage::External);
    llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );

    let emitted = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap();
    assert_eq!(
        emitted.registrations()[0].descriptor().get_name(),
        declaration.get_name()
    );
    assert!(declaration.get_initializer().is_some());

    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("already defined"), "{error}");
}

#[test]
fn rejects_missing_or_non_address_significant_callable_entries() {
    let (plan, surface) = callable_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("callable-registration-missing-entry");
    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("is not declared"), "{error}");
    assert!(
        llvm.get_global(expected.symbol().symbol().as_str())
            .is_none()
    );

    let llvm = context.create_module("callable-registration-unnamed-entry");
    let entry = llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );
    entry
        .as_global_value()
        .set_unnamed_address(UnnamedAddress::Global);
    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("address-significant"), "{error}");
    assert!(
        llvm.get_global(expected.symbol().symbol().as_str())
            .is_none()
    );
}

#[test]
fn rejects_incompatible_prior_declarations_and_entry_linkage() {
    let (plan, surface) = callable_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("callable-registration-collision");
    let symbol = expected.symbol().symbol();
    let incompatible = llvm.add_global(context.i8_type(), None, symbol.as_str());
    incompatible.set_linkage(Linkage::External);
    llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );
    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");

    let llvm = context.create_module("callable-registration-unnamed-declaration");
    let types = RuntimeMetadataV1Types::new(&context);
    let declaration = llvm.add_global(
        types.callable_registration_descriptor,
        None,
        symbol.as_str(),
    );
    declaration.set_linkage(Linkage::External);
    declaration.set_unnamed_address(UnnamedAddress::Local);
    llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );
    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");

    let llvm = context.create_module("callable-registration-private-entry");
    let entry = llvm.add_function(
        expected.entry_symbol().symbol().as_str(),
        context.void_type().fn_type(&[], false),
        None,
    );
    entry.set_linkage(Linkage::Private);
    let error = emit_strong_callable_registrations_v1(
        &context,
        &llvm,
        &plan,
        &surface,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        plan.registrations()[0].body(),
    )
    .unwrap_err();
    assert!(error.0.contains("external linkage"), "{error}");
}

fn constant_u64(value: inkwell::values::StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}

fn types(context: &Context) -> RuntimeMetadataV1Types<'_> {
    RuntimeMetadataV1Types::new(context)
}
