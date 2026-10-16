//! These tests cross-emit ELF using the linked LLVM on every compiler host.

use inkwell::module::Linkage;
use object::{Object, ObjectSection};

use super::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod bridges;
mod objects;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_eh;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_image;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_local_relocations;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_metadata;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_metadata_fixture;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_requirements;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_stackmaps;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_support;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod slib_types;

fn for_target(mut module: Module, target: scoop_lir::TargetProfileId) -> Module {
    module.meta = string_metadata_for(
        scoop_target_data::layout(target).expect("implemented target selected by this consumer"),
    );
    module
}

fn emit(module: &Module, optimization: OptimizationLevel, separate_sections: bool, path: &Path) {
    let profile = ValidatedBackendProfile::from_selection(
        scoop_target_data::selection(module.meta.target_profile.id())
            .expect("implemented target selected by this consumer"),
    )
    .expect("Linux backend")
    .with_optimization(if optimization == OptimizationLevel::None {
        scoop_lir::OptimizationMode::Debug
    } else {
        scoop_lir::OptimizationMode::Release
    });
    let machine = profile
        .create_qualification_target_machine(optimization)
        .expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, profile).expect("emit actual LIR");
    if separate_sections {
        for function in llvm
            .get_functions()
            .filter(|function| function.count_basic_blocks() != 0)
        {
            let name = function.get_name().to_str().unwrap();
            function.set_linkage(Linkage::WeakODR);
            function
                .as_global_value()
                .set_comdat(llvm.get_or_insert_comdat(name));
        }
    }
    let expected = statepoint::expectations(module).unwrap();
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    llvm.verify().expect("valid LLVM IR");
    statepoint::rewrite(&llvm, &machine, &expected, profile).expect("RS4GC");
    llvm.verify().expect("valid rewritten IR");

    machine
        .write_to_file(&llvm, FileType::Object, path)
        .expect("emit ELF");
}

#[test]
fn linux_amd64_statepoints_and_exceptions_at_o0_and_o2() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for optimization in [OptimizationLevel::None, OptimizationLevel::Default] {
            for separate_sections in [false, true] {
                for module in [
                    moving_gc::qualification::stackmap_qualification_module(),
                    exceptions_module(),
                ] {
                    let module = for_target(module, target);
                    let path = directory.path().join("code.o");
                    emit(&module, optimization, separate_sections, &path);
                    let bytes = std::fs::read(&path).unwrap();
                    let object = object::File::parse(bytes.as_slice()).unwrap();
                    assert_eq!(object.kind(), object::ObjectKind::Relocatable);
                    assert!(object.section_by_name(".llvm_stackmaps").unwrap().size() > 16);
                    assert!(object.section_by_name(".eh_frame").is_some());
                }
            }
        }
    }
}

#[test]
fn linux_amd64_requires_noredzone_on_rewritten_functions() {
    let module = for_target(
        moving_gc::qualification::stackmap_qualification_module(),
        scoop_target_data::LINUX_X86_64_GNU_ID,
    );
    let profile = ValidatedBackendProfile::from_selection(
        scoop_target_data::selection(module.meta.target_profile.id())
            .expect("implemented target selected by this consumer"),
    )
    .unwrap();
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    statepoint::rewrite(
        &llvm,
        &machine,
        &statepoint::expectations(&module).unwrap(),
        profile,
    )
    .unwrap();
    let function = llvm.get_function(module.functions[0].symbol()).unwrap();
    assert!(
        function
            .get_enum_attribute(
                AttributeLoc::Function,
                Attribute::get_named_enum_kind_id("noredzone"),
            )
            .is_some()
    );
}
