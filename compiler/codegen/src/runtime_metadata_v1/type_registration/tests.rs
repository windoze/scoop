use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::{GlobalValue, StructValue};
use scoop_identity::ConeIdentity;

use super::{
    TYPE_REGISTRATION_DESCRIPTOR_MAGIC, TYPE_REGISTRATION_DESCRIPTOR_SIZE,
    emit_strong_type_registrations_v1,
};
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

mod support;
use support::type_plan;

#[test]
fn emits_strong_record_with_type_descriptor() {
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

    assert_eq!(registration.primary_atom(), expected.primary_atom());

    let initializer = descriptor.get_initializer().unwrap().into_struct_value();
    let prefix = initializer
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(prefix, 0), TYPE_REGISTRATION_DESCRIPTOR_MAGIC);
    assert_eq!(constant_u64(prefix, 1), 9);
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
    for index in 3..=4 {
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
    llvm.verify().unwrap();
}

#[test]
fn completes_a_matching_image_declaration() {
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
}

#[test]
fn reports_missing_type_descriptors() {
    let plan = type_plan(1);
    let context = Context::create();

    let llvm = context.create_module("missing-type-descriptor");
    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("is not declared"), "{error}");
}

#[test]
fn reports_missing_registration_input() {
    let plan = type_plan(2);
    let context = Context::create();
    let llvm = context.create_module("missing-type-descriptor");
    declare_type_descriptor(&context, &llvm, plan.registrations()[0].descriptor_symbol());

    let error = emit_strong_type_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("is not declared"), "{error}");
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

fn constant_u64(value: StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}
