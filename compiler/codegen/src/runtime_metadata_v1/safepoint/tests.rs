use inkwell::context::Context;
use inkwell::module::Linkage;
use scoop_identity::ConeIdentity;

use super::{
    SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC, SAFEPOINT_REGISTRATION_DESCRIPTOR_SIZE,
    emit_strong_safepoint_registrations_v1,
};
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

mod support;
use support::safepoint_plan;

#[test]
fn emits_strong_record_with_owner_and_root_count() {
    let plan = safepoint_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("safepoint-registration");

    let emitted = emit_strong_safepoint_registrations_v1(
        &context,
        &llvm,
        &plan,
        plan.registrations()[0].owner(),
        |_| Some(plan.registrations()[0].root_pair_count() as usize),
    )
    .unwrap();

    assert_eq!(emitted.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(emitted.registrations().len(), 1);
    let registration = emitted.registrations()[0];
    assert_eq!(registration.site(), expected.site());
    let descriptor = registration.descriptor();
    assert_eq!(descriptor.get_linkage(), Linkage::External);
    assert!(descriptor.is_constant());
    assert_eq!(registration.primary_atom(), expected.primary_atom());

    let initializer = descriptor.get_initializer().unwrap().into_struct_value();
    let prefix = initializer
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(
        constant_u64(prefix, 0),
        SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC
    );
    assert_eq!(constant_u64(prefix, 1), 9);
    assert_eq!(
        constant_u64(prefix, 2),
        SAFEPOINT_REGISTRATION_DESCRIPTOR_SIZE
    );
    let identity = initializer
        .get_field_at_index(1)
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(identity, 0), 1);
    assert_eq!(constant_u64(identity, 1), 0);
    assert_eq!(
        identity.get_field_at_index(2).unwrap().into_struct_value(),
        super::digest_value(&context, types(&context).digest, expected.site().as_array())
    );
    assert!(
        identity
            .get_field_at_index(3)
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert!(
        identity
            .get_field_at_index(4)
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert_eq!(constant_u64(initializer, 2), expected.safepoint().get());
    assert_eq!(
        constant_u64(initializer, 3),
        u64::from(expected.role().tag())
    );
    assert_eq!(
        constant_u64(initializer, 4),
        u64::from(expected.root_pair_count())
    );
    assert_eq!(
        initializer
            .get_field_at_index(5)
            .unwrap()
            .into_struct_value(),
        super::digest_value(
            &context,
            types(&context).digest,
            expected.owner().as_array()
        )
    );
    llvm.verify().unwrap();
}

#[test]
fn completes_one_matching_image_declaration() {
    let plan = safepoint_plan();
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("safepoint-registration-declaration");
    let types = RuntimeMetadataV1Types::new(&context);
    let symbol = expected.symbol().symbol();
    let declaration = llvm.add_global(
        types.safepoint_registration_descriptor,
        None,
        symbol.as_str(),
    );
    declaration.set_linkage(Linkage::External);

    let emitted = emit_strong_safepoint_registrations_v1(
        &context,
        &llvm,
        &plan,
        plan.registrations()[0].owner(),
        |_| Some(plan.registrations()[0].root_pair_count() as usize),
    )
    .unwrap();
    assert_eq!(
        emitted.registrations()[0].descriptor().get_name(),
        declaration.get_name()
    );
    assert!(declaration.get_initializer().is_some());
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
