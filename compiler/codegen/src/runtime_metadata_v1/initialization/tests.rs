mod support;

use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::{GlobalValue, StructValue, UnnamedAddress};

use super::{
    DIGEST_SIZE, GATEWAY_DEFINITION_FINGERPRINT_OFFSET, INITIALIZATION_UNIT_DESCRIPTOR_MAGIC,
    INITIALIZATION_UNIT_DESCRIPTOR_SIZE, METADATA_ABI_VERSION,
    REGISTRATION_DEFINITION_FINGERPRINT_OFFSET, emit_strong_initialization_unit_registrations_v1,
};
use support::initialization_plan;

#[test]
fn emits_one_registration_with_its_cell_and_diagnostic() {
    let plan = initialization_plan(false);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("initialization-registration");
    declare_prerequisites(&context, &llvm, expected);

    let emitted = emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap();

    assert_eq!(emitted.producer(), plan.producer());
    assert_eq!(emitted.registrations().len(), 1);
    let registration = emitted.registrations()[0];
    assert_eq!(registration.unit(), expected.semantic().unit());
    assert_eq!(registration.cell().get_linkage(), Linkage::External);
    assert!(!registration.cell().is_constant());
    assert_eq!(
        registration.cell().get_section().unwrap().to_str().unwrap(),
        "__DATA,__data"
    );
    assert!(
        registration
            .cell()
            .get_initializer()
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert_eq!(
        name(registration.cell()),
        expected.cell_symbol().symbol().as_str()
    );
    assert_eq!(
        name(registration.registration_descriptor()),
        expected.registration_symbol().symbol().as_str()
    );
    assert_ne!(
        name(registration.cell()),
        name(registration.registration_descriptor())
    );
    assert_eq!(registration.diagnostic_atom(), expected.diagnostic_atom());
    assert!(registration.diagnostic().is_constant());
    assert_eq!(
        registration
            .diagnostic()
            .get_section()
            .unwrap()
            .to_str()
            .unwrap(),
        "__TEXT,__cstring,cstring_literals"
    );
    assert!(registration.registration_descriptor().is_constant());

    let descriptor = registration
        .registration_descriptor()
        .get_initializer()
        .unwrap()
        .into_struct_value();
    let prefix = descriptor
        .get_field_at_index(0)
        .unwrap()
        .into_struct_value();
    assert_eq!(
        constant_u64(prefix, 0),
        INITIALIZATION_UNIT_DESCRIPTOR_MAGIC
    );
    assert_eq!(constant_u64(prefix, 1), METADATA_ABI_VERSION);
    assert_eq!(constant_u64(prefix, 2), INITIALIZATION_UNIT_DESCRIPTOR_SIZE);
    assert_eq!(constant_u64(descriptor, 2), 1);
    assert_eq!(
        descriptor
            .get_field_at_index(5)
            .unwrap()
            .into_pointer_value(),
        registration.cell().as_pointer_value()
    );
    assert_eq!(
        descriptor
            .get_field_at_index(10)
            .unwrap()
            .into_pointer_value(),
        llvm.get_function(expected.initializer().entry_symbol().symbol().as_str())
            .unwrap()
            .as_global_value()
            .as_pointer_value()
    );
    assert_eq!(
        descriptor
            .get_field_at_index(11)
            .unwrap()
            .into_pointer_value(),
        llvm.get_function(expected.ensure().entry_symbol().symbol().as_str())
            .unwrap()
            .as_global_value()
            .as_pointer_value()
    );
    assert_eq!(
        descriptor
            .get_field_at_index(14)
            .unwrap()
            .into_pointer_value(),
        llvm.get_function(
            expected
                .schedule()
                .gateway()
                .unwrap()
                .entry_symbol()
                .symbol()
                .as_str()
        )
        .unwrap()
        .as_global_value()
        .as_pointer_value()
    );

    let definition_patch = registration.registration_definition_patch();
    assert_eq!(
        definition_patch.intent(),
        expected.registration_definition_patch()
    );
    assert_eq!(
        definition_patch.definition(),
        expected.registration_definition_plan()
    );
    assert_eq!(
        definition_patch.atom(),
        expected.registration_primary_atom()
    );
    assert_eq!(
        name(definition_patch.owner()),
        name(registration.registration_descriptor())
    );
    assert_eq!(
        definition_patch.byte_offset(),
        REGISTRATION_DEFINITION_FINGERPRINT_OFFSET
    );
    assert_eq!(definition_patch.byte_size(), DIGEST_SIZE);
    let gateway_patch = registration.gateway_definition_patch().unwrap();
    assert_eq!(
        gateway_patch.intent(),
        expected.schedule().gateway_definition_patch().unwrap()
    );
    assert_eq!(
        gateway_patch.definition(),
        expected.registration_definition_plan()
    );
    assert_eq!(gateway_patch.atom(), expected.registration_primary_atom());
    assert_eq!(
        gateway_patch.byte_offset(),
        GATEWAY_DEFINITION_FINGERPRINT_OFFSET
    );
    assert_eq!(gateway_patch.byte_size(), DIGEST_SIZE);

    let ir = llvm.print_to_string().to_string();
    assert!(!ir.contains("scoop_image_initialization_units"));
    assert!(!ir.contains("scoop.init.cell."));
    assert!(!ir.contains("scoop.init.descriptor."));
    assert!(!ir.contains("scoop$1$id$"));
    llvm.verify().unwrap();
}

#[test]
fn lazy_registration_has_no_gateway_or_gateway_patch() {
    let plan = initialization_plan(true);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("lazy-initialization-registration");
    declare_prerequisites(&context, &llvm, expected);

    let emitted = emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap();
    let registration = emitted.registrations()[0];

    assert!(registration.gateway_definition_patch().is_none());
    let descriptor = registration
        .registration_descriptor()
        .get_initializer()
        .unwrap()
        .into_struct_value();
    assert_eq!(constant_u64(descriptor, 2), 2);
    assert!(
        descriptor
            .get_field_at_index(12)
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert!(
        descriptor
            .get_field_at_index(13)
            .unwrap()
            .into_struct_value()
            .is_null()
    );
    assert!(
        descriptor
            .get_field_at_index(14)
            .unwrap()
            .into_pointer_value()
            .is_null()
    );
    llvm.verify().unwrap();
}

#[test]
fn validates_all_prerequisites_before_emitting_owned_definitions() {
    let plan = initialization_plan(false);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("missing-initialization-prerequisite");
    declare_storage(&context, &llvm, expected.storage().storage_symbol());
    declare_storage(&context, &llvm, expected.failure_root().storage_symbol());
    declare_callable(
        &context,
        &llvm,
        expected.initializer(),
        CallableKind::Managed,
    );

    let error = emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap_err();

    assert!(error.0.contains("is not declared"), "{error}");
    assert_owned_definitions_absent(&llvm, expected);
}

#[test]
fn rejects_incompatible_prerequisites_and_owned_redefinition() {
    let plan = initialization_plan(false);
    let expected = &plan.registrations()[0];
    let context = Context::create();
    let llvm = context.create_module("incompatible-initialization-callable");
    declare_storage_set(&context, &llvm, expected);
    let wrong = llvm.add_function(
        expected.initializer().entry_symbol().symbol().as_str(),
        context.i32_type().fn_type(&[], false),
        None,
    );
    wrong.set_linkage(Linkage::External);
    declare_callable(&context, &llvm, expected.ensure(), CallableKind::Managed);
    declare_callable(
        &context,
        &llvm,
        *expected.schedule().gateway().unwrap(),
        CallableKind::Gateway,
    );
    let error = emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap_err();
    assert!(
        error.0.contains("incompatible entry declaration"),
        "{error}"
    );
    assert_owned_definitions_absent(&llvm, expected);

    let llvm = context.create_module("initialization-owned-redefinition");
    declare_prerequisites(&context, &llvm, expected);
    emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap();
    let error = emit_strong_initialization_unit_registrations_v1(
        &context,
        &llvm,
        crate::target::ValidatedBackendProfile::darwin_aarch64_for_test(),
        &plan,
    )
    .unwrap_err();
    assert!(error.0.contains("already defined"), "{error}");
}

fn declare_prerequisites<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlanV1,
) {
    declare_storage_set(context, llvm, plan);
    declare_callable(context, llvm, plan.initializer(), CallableKind::Managed);
    declare_callable(context, llvm, plan.ensure(), CallableKind::Managed);
    if let Some(gateway) = plan.schedule().gateway() {
        declare_callable(context, llvm, *gateway, CallableKind::Gateway);
    }
}

fn declare_storage_set<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlanV1,
) {
    declare_storage(context, llvm, plan.storage().storage_symbol());
    declare_storage(context, llvm, plan.failure_root().storage_symbol());
}

fn declare_storage<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    request: scoop_lir::PersistentSymbolRequest,
) -> GlobalValue<'ctx> {
    let global = llvm.add_global(context.i64_type(), None, request.symbol().as_str());
    global.set_linkage(Linkage::External);
    global.set_constant(false);
    global.set_unnamed_address(UnnamedAddress::None);
    global.set_initializer(&context.i64_type().const_zero());
    global
}

#[derive(Clone, Copy)]
enum CallableKind {
    Managed,
    Gateway,
}

fn declare_callable<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: scoop_lir::StrongInitializationCallableRefPlanV1,
    kind: CallableKind,
) {
    let ty = match kind {
        CallableKind::Managed => context.void_type().fn_type(&[], false),
        CallableKind::Gateway => context.i32_type().fn_type(&[], false),
    };
    let function = llvm.add_function(plan.entry_symbol().symbol().as_str(), ty, None);
    function.set_linkage(Linkage::External);
    function
        .as_global_value()
        .set_unnamed_address(UnnamedAddress::None);
}

fn assert_owned_definitions_absent(
    llvm: &inkwell::module::Module<'_>,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlanV1,
) {
    for request in [plan.cell_symbol(), plan.registration_symbol()] {
        assert!(llvm.get_global(request.symbol().as_str()).is_none());
    }
    assert!(
        llvm.get_global(&format!(
            "{}.diagnostic",
            plan.registration_symbol().symbol()
        ))
        .is_none()
    );
}

fn constant_u64(value: StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}

fn name(global: GlobalValue<'_>) -> String {
    global.get_name().to_str().unwrap().to_string()
}
