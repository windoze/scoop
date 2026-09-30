use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::targets::TargetData;
use inkwell::values::{AnyValue, GlobalValue, StructValue};
use scoop_lir::{LirTargetProfile, StaticStorageScanKindV1, StrongStaticStorageInitialStatePlanV1};

use super::{
    DIGEST_SIZE, EMPTY_RELOCATION_SENTINEL, EMPTY_TEMPLATE_SENTINEL,
    EmittedStaticStorageInitialStateV1, EmittedStaticStorageRelocationTableV1,
    LAYOUT_FINGERPRINT_OFFSET, REGISTRATION_DEFINITION_FINGERPRINT_OFFSET, SCAN_FINGERPRINT_OFFSET,
    STATIC_STORAGE_DESCRIPTOR_MAGIC, STATIC_STORAGE_DESCRIPTOR_SIZE,
    emit_strong_static_storage_registrations_v1,
};
use crate::ManagedAddressSpace;

mod support;
use support::static_storage_plan;

#[test]
fn emits_closed_descriptors_scans_initial_state_and_zero_patch_sites() {
    let plan = static_storage_plan();
    let context = Context::create();
    let llvm = context.create_module("static-storage-registration");
    declare_storage_set(&context, &llvm, &plan);
    let target_data = target_data();

    let emitted =
        emit_strong_static_storage_registrations_v1(&context, &llvm, &target_data, &plan).unwrap();

    assert_eq!(emitted.producer(), plan.producer());
    assert_eq!(emitted.registrations().len(), 2);
    assert!(
        emitted
            .registrations()
            .windows(2)
            .all(|pair| { pair[0].storage() < pair[1].storage() })
    );
    for (registration, expected) in emitted.registrations().iter().zip(plan.registrations()) {
        assert_eq!(registration.storage(), expected.semantic().storage());
        assert_eq!(registration.descriptor().get_linkage(), Linkage::External);
        assert!(registration.descriptor().is_constant());
        assert_eq!(
            registration.storage_value().get_linkage(),
            Linkage::External
        );
        assert!(!registration.storage_value().is_constant());

        let initializer = registration
            .descriptor()
            .get_initializer()
            .unwrap()
            .into_struct_value();
        let prefix = initializer
            .get_field_at_index(0)
            .unwrap()
            .into_struct_value();
        assert_eq!(constant_u64(prefix, 0), STATIC_STORAGE_DESCRIPTOR_MAGIC);
        assert_eq!(constant_u64(prefix, 1), 2);
        assert_eq!(constant_u64(prefix, 2), STATIC_STORAGE_DESCRIPTOR_SIZE);
        assert_eq!(
            constant_u64(initializer, 2),
            u64::from(expected.semantic().scan_kind().tag())
        );
        assert_eq!(
            constant_u64(initializer, 3),
            u64::from(expected.semantic().initial_state().tag())
        );
        assert_eq!(
            constant_u64(initializer, 5),
            expected.semantic().byte_size()
        );
        assert_eq!(
            constant_u64(initializer, 6),
            expected.semantic().allocation_extent()
        );
        assert_eq!(
            constant_u64(initializer, 7),
            expected.semantic().required_alignment()
        );
        assert_eq!(
            constant_u64(initializer, 13),
            expected
                .semantic()
                .initial_state()
                .immortal_relocations()
                .len() as u64
        );

        let patches = [
            (
                registration.registration_definition_patch(),
                expected.registration_definition_patch(),
                REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
            ),
            (
                registration.scan_fingerprint_patch(),
                expected.scan_fingerprint_patch(),
                SCAN_FINGERPRINT_OFFSET,
            ),
            (
                registration.layout_fingerprint_patch(),
                expected.layout_fingerprint_patch(),
                LAYOUT_FINGERPRINT_OFFSET,
            ),
        ];
        for (patch, intent, offset) in patches {
            assert_eq!(patch.intent(), intent);
            assert_eq!(patch.definition(), expected.registration_definition_plan());
            assert_eq!(patch.atom(), expected.registration_primary_atom());
            assert_eq!(
                patch.owner().get_name(),
                registration.descriptor().get_name()
            );
            assert_eq!(patch.byte_offset(), offset);
            assert_eq!(patch.byte_size(), DIGEST_SIZE);
        }

        let scan = registration
            .scan_program()
            .get_initializer()
            .unwrap()
            .into_array_value();
        match expected.semantic().scan_kind() {
            StaticStorageScanKindV1::None => {
                assert_eq!(scan.get_type().len(), 1);
                assert!(
                    scan.print_to_string()
                        .to_string()
                        .contains("zeroinitializer")
                );
            }
            StaticStorageScanKindV1::Recursive => {
                assert_eq!(scan.get_type().len(), 2);
                assert!(
                    scan.print_to_string()
                        .to_string()
                        .contains("[i64 1, i64 0]")
                );
            }
        }

        match (
            registration.initial_state(),
            expected.semantic().initial_state(),
        ) {
            (
                EmittedStaticStorageInitialStateV1::ZeroedForRuntimeUnit {
                    template_sentinel,
                    relocations:
                        EmittedStaticStorageRelocationTableV1::SharedEmptySentinel { global },
                },
                StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit,
            ) => {
                assert_eq!(
                    template_sentinel.get_name().to_str().unwrap(),
                    EMPTY_TEMPLATE_SENTINEL
                );
                assert_eq!(
                    global.get_name().to_str().unwrap(),
                    EMPTY_RELOCATION_SENTINEL
                );
            }
            (
                EmittedStaticStorageInitialStateV1::EncodedStaticValue {
                    template,
                    relocations: EmittedStaticStorageRelocationTableV1::Defined { global, .. },
                    ..
                },
                StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                    initial_template,
                    immortal_relocations,
                },
            ) => {
                assert_eq!(
                    template
                        .get_initializer()
                        .unwrap()
                        .into_array_value()
                        .get_type()
                        .len() as usize,
                    initial_template.len()
                );
                assert_eq!(
                    global
                        .get_initializer()
                        .unwrap()
                        .into_array_value()
                        .get_type()
                        .len() as usize,
                    immortal_relocations.len()
                );
            }
            actual => panic!("unexpected initial-state pairing: {actual:?}"),
        }
    }
    assert!(
        llvm.get_global(EMPTY_TEMPLATE_SENTINEL)
            .unwrap()
            .is_constant()
    );
    assert!(
        llvm.get_global(EMPTY_RELOCATION_SENTINEL)
            .unwrap()
            .is_constant()
    );
    llvm.verify().unwrap();
}

#[test]
fn validates_the_complete_storage_set_before_emitting_metadata() {
    let plan = static_storage_plan();
    let context = Context::create();
    let llvm = context.create_module("atomic-static-storage-validation");
    declare_storage(&context, &llvm, &plan.registrations()[0]);

    let error = emit_strong_static_storage_registrations_v1(&context, &llvm, &target_data(), &plan)
        .unwrap_err();

    assert!(error.0.contains("is not defined"), "{error}");
    assert!(llvm.get_global(EMPTY_TEMPLATE_SENTINEL).is_none());
    assert!(llvm.get_global(EMPTY_RELOCATION_SENTINEL).is_none());
    for registration in plan.registrations() {
        assert!(
            llvm.get_global(registration.registration_symbol().symbol().as_str())
                .is_none()
        );
        assert!(
            llvm.get_global(registration.scan_symbol().symbol().as_str())
                .is_none()
        );
    }
}

#[test]
fn rejects_wrong_storage_shape_and_descriptor_redefinition() {
    let plan = static_storage_plan();
    let context = Context::create();
    let llvm = context.create_module("wrong-static-storage-shape");
    declare_storage_set(&context, &llvm, &plan);
    let first = llvm
        .get_global(
            plan.registrations()[0]
                .semantic()
                .symbol()
                .symbol()
                .as_str(),
        )
        .unwrap();
    first.set_constant(true);
    let error = emit_strong_static_storage_registrations_v1(&context, &llvm, &target_data(), &plan)
        .unwrap_err();
    assert!(error.0.contains("writable address-significant"), "{error}");

    first.set_constant(false);
    emit_strong_static_storage_registrations_v1(&context, &llvm, &target_data(), &plan).unwrap();
    let error = emit_strong_static_storage_registrations_v1(&context, &llvm, &target_data(), &plan)
        .unwrap_err();
    assert!(error.0.contains("already defined"), "{error}");
}

fn declare_storage_set<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: &scoop_lir::StrongStaticStorageRegistrationPlanSetV1,
) {
    for registration in plan.registrations() {
        declare_storage(context, llvm, registration);
    }
}

fn declare_storage<'ctx>(
    context: &'ctx Context,
    llvm: &inkwell::module::Module<'ctx>,
    plan: &scoop_lir::StrongStaticStorageRegistrationPlanV1,
) -> GlobalValue<'ctx> {
    let semantic = plan.semantic();
    let global = match semantic.scan_kind() {
        StaticStorageScanKindV1::None => {
            let global = llvm.add_global(
                context.i64_type(),
                None,
                semantic.symbol().symbol().as_str(),
            );
            global.set_initializer(&context.i64_type().const_zero());
            global
        }
        StaticStorageScanKindV1::Recursive => {
            let ty = context.ptr_type(ManagedAddressSpace::MOVING_GC.inkwell());
            let global = llvm.add_global(ty, None, semantic.symbol().symbol().as_str());
            global.set_initializer(&ty.const_null());
            global
        }
    };
    global.set_linkage(Linkage::External);
    global.set_constant(false);
    global.set_alignment(semantic.required_alignment() as u32);
    assert_eq!(
        global.as_pointer_value().get_type().get_address_space(),
        AddressSpace::default()
    );
    global
}

fn target_data() -> TargetData {
    TargetData::create(LirTargetProfile::DARWIN_AARCH64.canonical_llvm_data_layout())
}

fn constant_u64(value: StructValue<'_>, index: u32) -> u64 {
    value
        .get_field_at_index(index)
        .unwrap()
        .into_int_value()
        .get_zero_extended_constant()
        .unwrap()
}
