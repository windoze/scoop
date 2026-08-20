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
    module.meta = string_metadata_for(scoop_lir::LirTargetProfile::from_id(target));
    module
}

fn emit(
    module: &Module,
    optimization: OptimizationLevel,
    separate_sections: bool,
    path: &Path,
) -> (ValidatedBackendProfile, statepoint::ExpectedSafepoints) {
    let profile = ValidatedBackendProfile::from_selection(
        scoop_lir::ValidatedLirTargetSelection::from_id(module.meta.target_profile.id()),
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
    let expected = statepoint::rewrite(&llvm, &machine, &expected, profile).expect("RS4GC");
    llvm.verify().expect("valid rewritten IR");
    statepoint::verify_rewritten(&llvm, &expected, profile).expect("machine policy and roots");
    machine
        .write_to_file(&llvm, FileType::Object, path)
        .expect("emit ELF");
    (profile, expected)
}

fn check(
    module: &Module,
    expected: &statepoint::ExpectedSafepoints,
    profile: ValidatedBackendProfile,
    path: &Path,
) -> Result<(), CodegenError> {
    profile.verify_object(path, expected, &artifact::eh_expectations(module)?)
}

#[test]
fn linux_amd64_statepoints_and_exceptions_at_o0_and_o2() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for optimization in [OptimizationLevel::None, OptimizationLevel::Default] {
            for separate_sections in [false, true] {
                for module in [
                    moving_gc::qualification::stackmap_qualification_module(),
                    exceptions_module(),
                ] {
                    let module = for_target(module, target);
                    let path = directory.path().join("code.o");
                    let (profile, expected) = emit(&module, optimization, separate_sections, &path);
                    check(&module, &expected, profile, &path).unwrap_or_else(|error| {
                        panic!("{target:?}/{optimization:?}/COMDAT={separate_sections}: {error}")
                    });
                }
            }
        }
    }
}

#[test]
fn linux_amd64_rejects_damaged_stackmap_and_eh_pointers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("code.o");
    let module = for_target(exceptions_module(), TargetProfileId::LinuxX86_64Gnu);
    let (profile, expected) = emit(&module, OptimizationLevel::None, true, &path);
    check(&module, &expected, profile, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    let stackmap = file.section_by_name(".llvm_stackmaps").unwrap();
    let offset = stackmap.file_range().unwrap().0 as usize;
    let mut damaged = bytes.clone();
    // The first function's frame size cannot include the amd64 saved return PC.
    damaged[offset + 24..offset + 32].copy_from_slice(&16u64.to_le_bytes());
    std::fs::write(&path, damaged).unwrap();
    assert!(
        check(&module, &expected, profile, &path)
            .unwrap_err()
            .0
            .contains("invalid stack size")
    );

    let eh = file.section_by_name(".eh_frame").unwrap();
    let personality_field = eh.relocations().map(|(offset, _)| offset).min().unwrap();
    let field = eh.file_range().unwrap().0 as usize + personality_field as usize;
    let mut damaged = bytes.clone();
    damaged[field] = 1;
    std::fs::write(&path, damaged).unwrap();
    assert!(
        check(&module, &expected, profile, &path)
            .unwrap_err()
            .0
            .contains("nonzero in-place addend")
    );

    let relocations = file.section_by_name(".rela.eh_frame").unwrap();
    let field = relocations.file_range().unwrap().0 as usize + 8;
    let mut damaged = bytes.clone();
    damaged[field..field + 4].copy_from_slice(&object::elf::R_X86_64_64.to_le_bytes());
    std::fs::write(&path, damaged).unwrap();
    assert!(
        check(&module, &expected, profile, &path)
            .unwrap_err()
            .0
            .contains("unexpected relocation")
    );
}

#[test]
fn linux_amd64_requires_noredzone_on_rewritten_functions() {
    let module = for_target(
        moving_gc::qualification::stackmap_qualification_module(),
        TargetProfileId::LinuxX86_64Gnu,
    );
    let profile = ValidatedBackendProfile::from_selection(
        scoop_lir::ValidatedLirTargetSelection::from_id(module.meta.target_profile.id()),
    )
    .unwrap();
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    let expected = statepoint::rewrite(
        &llvm,
        &machine,
        &statepoint::expectations(&module).unwrap(),
        profile,
    )
    .unwrap();
    let function = llvm.get_function(module.functions[0].symbol()).unwrap();
    function.remove_enum_attribute(
        AttributeLoc::Function,
        Attribute::get_named_enum_kind_id("noredzone"),
    );
    let error = statepoint::verify_rewritten(&llvm, &expected, profile).unwrap_err();
    assert!(error.0.contains("noredzone"), "{error}");
}
