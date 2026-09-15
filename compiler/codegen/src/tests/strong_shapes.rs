use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::targets::TargetData;
use scoop_lir::{StrongDefinitionRole, StrongObjectSymbolSurfaceV1};

use super::*;

#[test]
fn emits_every_canonical_global_shape_atom_and_boundary() {
    let module = heap_module();
    let foundation = scoop_lir::OdrFreeLirFoundation::from_module(&module)
        .expect("test module has a strong foundation");
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation)
        .expect("test module has a canonical symbol surface");
    let context = Context::create();
    let llvm = context.create_module("strong-shapes");
    let target_data = TargetData::create(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64.canonical_llvm_data_layout(),
    );
    let descriptor_type =
        runtime_metadata_v1::RuntimeMetadataV1Types::new(&context).type_descriptor();
    let type_globals = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(descriptor_type, None, descriptor.identity.symbol());
            global.set_linkage(Linkage::External);
            global.set_constant(true);
            global
        })
        .collect::<Vec<_>>();
    let external_type_globals = module
        .meta
        .core_external_type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(
                descriptor_type,
                None,
                descriptor.expected_symbol().symbol().as_str(),
            );
            global.set_linkage(Linkage::External);
            global
        })
        .collect::<Vec<_>>();
    for function in &module.functions {
        let function = llvm.add_function(
            function.symbol(),
            context.ptr_type(inkwell::AddressSpace::from(1u16)).fn_type(
                &[context.ptr_type(inkwell::AddressSpace::from(1u16)).into()],
                false,
            ),
            None,
        );
        function.set_linkage(Linkage::External);
    }

    shape_definitions::emit_strong_shape_definitions_v1(
        &context,
        &llvm,
        &target_data,
        &surface,
        &module,
        &type_globals,
        &external_type_globals,
    )
    .expect("emit canonical strong shape definitions");
    llvm.verify().expect("valid shape LLVM module");

    let ir = llvm.print_to_string().to_string();
    let shape_roles = [
        StrongDefinitionRole::TypeDescriptor,
        StrongDefinitionRole::Layout,
        StrongDefinitionRole::ScanProgram,
        StrongDefinitionRole::DispatchTable,
    ];
    for plan in surface
        .plans()
        .iter()
        .filter(|plan| shape_roles.contains(&plan.definition_role()))
    {
        let symbol = plan.primary_symbol().symbol();
        assert!(
            llvm.get_global(symbol.as_str()).is_some(),
            "missing primary shape definition `{symbol}`\n{ir}"
        );
        for boundary in plan.atom_boundaries() {
            for symbol in [boundary.start().symbol(), boundary.end().symbol()] {
                assert!(
                    ir.contains(&format!("@\"{symbol}\" = alias")),
                    "missing boundary alias `{symbol}`\n{ir}"
                );
            }
        }
    }
    assert!(!ir.contains(".object_scan = private"), "{ir}");
    assert!(!ir.contains(".vtable = private"), "{ir}");
}

#[test]
fn rejects_array_descriptor_without_its_exact_element_scan_definition() {
    let mut module = values_module();
    let array = array_type(
        &mut module.meta,
        "ArrayRefShape",
        scoop_lir::ArrayKind::Immutable,
        MANAGED_PTR,
        8,
        8,
        RefScan::References(vec![0]),
    );
    module.meta.arrays[array].element_scan = RefScan::None;

    let validation_error = validation::validate_module(&module)
        .expect_err("array metadata and descriptor shape must agree before emission");
    assert!(
        validation_error
            .0
            .contains("does not match its closed element size, alignment, and scan shape"),
        "{validation_error}"
    );

    let error = try_strong_shape_ir_of(&module)
        .expect_err("descriptor inline scans require a nonempty typed LIR scan definition");
    assert!(
        error.0.contains("references empty typed inline scan"),
        "{error}"
    );
}
