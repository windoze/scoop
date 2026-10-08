use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::{GlobalValue, StructValue};
use scoop_identity::ConeIdentity;
use scoop_lir::StrongImmortalObjectRegistrationPlanV1;

use super::{
    IMMORTAL_OBJECT_DESCRIPTOR_MAGIC, IMMORTAL_OBJECT_DESCRIPTOR_SIZE,
    emit_strong_immortal_object_registrations_v1,
};
use crate::ManagedAddressSpace;
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

mod support;
use support::immortal_plan;

#[test]
fn emits_closed_strong_descriptor_without_digest_patch_sites() {
    let plan = immortal_plan(1);
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("immortal-registration");
    let object = declare_immortal_object(&context, &llvm, expected);

    let emitted = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap();

    assert_eq!(emitted.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(emitted.registrations().len(), 1);
    let registration = emitted.registrations()[0];
    assert_eq!(registration.object(), expected.object());
    assert_eq!(registration.object_value().get_name(), object.get_name());
    let descriptor = registration.descriptor();
    assert_eq!(descriptor.get_linkage(), Linkage::External);
    assert!(descriptor.is_constant());
    let initializer = descriptor.get_initializer().unwrap().into_struct_value();
    let prefix = initializer
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(prefix, 0), IMMORTAL_OBJECT_DESCRIPTOR_MAGIC);
    assert_eq!(constant_u64(prefix, 1), 7);
    assert_eq!(constant_u64(prefix, 2), IMMORTAL_OBJECT_DESCRIPTOR_SIZE);
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
            expected.object().as_array(),
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
    assert_eq!(
        initializer
            .get_field_at_index(2)
            .unwrap()
            .into_pointer_value(),
        object
            .as_pointer_value()
            .const_address_space_cast(context.ptr_type(AddressSpace::default()))
    );
    assert_eq!(constant_u64(initializer, 3), expected.object_size());
    assert_eq!(constant_u64(initializer, 4), expected.required_alignment());
    assert_eq!(
        initializer
            .get_field_at_index(5)
            .unwrap()
            .into_pointer_value(),
        registration.type_registration().as_pointer_value()
    );
    assert_eq!(
        registration
            .type_registration()
            .get_name()
            .to_str()
            .unwrap(),
        expected.type_registration_symbol().symbol().as_str()
    );
    assert!(registration.type_registration().get_initializer().is_none());
    llvm.verify().unwrap();
}

#[test]
fn completes_matching_declaration_then_rejects_redefinition() {
    let plan = immortal_plan(1);
    let expected = plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("immortal-registration-declaration");
    declare_immortal_object(&context, &llvm, expected);
    let types = RuntimeMetadataV1Types::new(&context);
    let type_registration = llvm.add_global(
        types.type_registration_descriptor,
        None,
        expected.type_registration_symbol().symbol().as_str(),
    );
    type_registration.set_linkage(Linkage::External);
    let declaration = llvm.add_global(
        types.immortal_object_descriptor,
        None,
        expected.registration_symbol().symbol().as_str(),
    );
    declaration.set_linkage(Linkage::External);

    let emitted = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap();
    assert_eq!(
        emitted.registrations()[0].descriptor().get_name(),
        declaration.get_name()
    );
    assert_eq!(
        emitted.registrations()[0].type_registration().get_name(),
        type_registration.get_name()
    );
    assert!(declaration.get_initializer().is_some());

    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("already defined"), "{error}");
}

#[test]
fn reuses_one_external_type_registration_for_all_objects() {
    let plan = immortal_plan(2);
    let context = Context::create();
    let llvm = context.create_module("shared-immortal-type-registration");
    for registration in plan.registrations() {
        declare_immortal_object(&context, &llvm, *registration);
    }

    let emitted = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap();

    let first = emitted.registrations()[0].type_registration();
    let second = emitted.registrations()[1].type_registration();
    assert_eq!(first.get_name(), second.get_name());
    let symbol = plan.registrations()[0].type_registration_symbol().symbol();
    assert_eq!(first.get_name().to_str().unwrap(), symbol.as_str());
    assert!(llvm.get_global(&format!("{symbol}.1")).is_none());
    llvm.verify().unwrap();
}

#[test]
fn validates_the_complete_set_before_emitting_any_registration() {
    let plan = immortal_plan(2);
    let context = Context::create();
    let llvm = context.create_module("atomic-immortal-registration-validation");
    declare_immortal_object(&context, &llvm, plan.registrations()[0]);

    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();

    assert!(error.0.contains("is not defined"), "{error}");
    for registration in plan.registrations() {
        assert!(
            llvm.get_global(registration.registration_symbol().symbol().as_str())
                .is_none()
        );
        assert!(
            llvm.get_global(registration.type_registration_symbol().symbol().as_str())
                .is_none()
        );
    }
}

#[test]
fn rejects_mutable_objects_and_incompatible_metadata_declarations() {
    let plan = immortal_plan(1);
    let expected = plan.registrations()[0];
    let context = Context::create();

    let llvm = context.create_module("mutable-immortal-object");
    let object = declare_immortal_object(&context, &llvm, expected);
    object.set_constant(false);
    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("constant address-significant"), "{error}");

    let llvm = context.create_module("raw-address-immortal-object");
    let bytes = context.i8_type().array_type(expected.object_size() as u32);
    let object = llvm.add_global(bytes, None, expected.object_symbol().symbol().as_str());
    object.set_linkage(Linkage::External);
    object.set_constant(true);
    object.set_initializer(&bytes.const_zero());
    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("managed LLVM address space"), "{error}");

    let llvm = context.create_module("wrong-type-registration");
    declare_immortal_object(&context, &llvm, expected);
    let wrong_type = llvm.add_global(
        context.i8_type(),
        None,
        expected.type_registration_symbol().symbol().as_str(),
    );
    wrong_type.set_linkage(Linkage::External);
    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");

    let llvm = context.create_module("wrong-immortal-registration");
    declare_immortal_object(&context, &llvm, expected);
    let wrong_registration = llvm.add_global(
        context.i8_type(),
        None,
        expected.registration_symbol().symbol().as_str(),
    );
    wrong_registration.set_linkage(Linkage::External);
    let error = emit_strong_immortal_object_registrations_v1(&context, &llvm, &plan).unwrap_err();
    assert!(error.0.contains("incompatible LLVM declaration"), "{error}");
}

fn declare_immortal_object<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> GlobalValue<'ctx> {
    let bytes = context.i8_type().array_type(plan.object_size() as u32);
    let object = llvm.add_global(
        bytes,
        Some(ManagedAddressSpace::MOVING_GC.inkwell()),
        plan.object_symbol().symbol().as_str(),
    );
    object.set_linkage(Linkage::External);
    object.set_constant(true);
    object.set_alignment(plan.required_alignment() as u32);
    object.set_initializer(&bytes.const_zero());
    object
}

fn constant_u64(value: StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}
