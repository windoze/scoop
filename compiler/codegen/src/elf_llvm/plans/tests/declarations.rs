use std::collections::BTreeSet;
use std::path::Path;

use inkwell::memory_buffer::MemoryBuffer;
use inkwell::targets::FileType;
use llvm_sys::core::{LLVMGetFirstUse, LLVMGetUser, LLVMIsAConstantExpr};
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolFlags, elf};

use super::*;

fn symbol_plans() -> SymbolPlanIndex {
    SymbolPlanIndex {
        groups: vec!["owned".into(), "owned_odr".into(), "dead_planned".into()],
        symbols: [
            ("owned", 0, LinkageClass::ConeStrong),
            ("owned_odr", 1, LinkageClass::OdrWeak),
            ("projection", 1, LinkageClass::OdrWeak),
            ("dead_planned", 2, LinkageClass::OdrWeak),
        ]
        .into_iter()
        .enumerate()
        .map(|(order, (name, group, linkage))| {
            (
                name.as_bytes().to_vec(),
                vec![SymbolPlan {
                    order,
                    group,
                    linkage,
                    dispatch: false,
                }],
            )
        })
        .collect(),
    }
}

#[test]
fn optimized_declarations_do_not_leave_hidden_undefined_elf_symbols() {
    let directory = tempfile::tempdir().unwrap();
    let plans = symbol_plans();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [
            scoop_lir::OptimizationMode::Debug,
            scoop_lir::OptimizationMode::Release,
        ] {
            let profile = crate::target::ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = context
                .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                    include_bytes!("declarations.ll"),
                    "declarations",
                ))
                .unwrap();
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            plans.prepare(&llvm).unwrap();
            llvm.get_function("main")
                .unwrap()
                .run_passes(
                    "sccp,unreachableblockelim",
                    &machine,
                    PassBuilderOptions::create(),
                )
                .unwrap();

            // LLVM 22 retains these orphan GEP constants after removing the block.
            // The imported declaration is deliberately absent from symbol_plans().
            for name in ["dead_external", "dead_planned"] {
                let value = llvm.get_global(name).unwrap();
                unsafe {
                    let usage = LLVMGetFirstUse(value.as_value_ref());
                    assert!(!usage.is_null());
                    let user = LLVMGetUser(usage);
                    assert!(!LLVMIsAConstantExpr(user).is_null());
                    assert!(LLVMGetFirstUse(user).is_null());
                }
            }
            assert!(unsafe {
                LLVMGetFirstUse(llvm.get_global("dead_direct").unwrap().as_value_ref()).is_null()
            });

            plans.prepare_callable(&llvm, &machine).unwrap();
            llvm.verify().unwrap();
            let path = directory.path().join("declarations.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            verify_object(&path);

            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            if target == scoop_target_data::LINUX_X86_64_GNU_ID {
                link_and_run(directory.path(), &path);
            }
        }
    }
}

fn verify_object(path: &Path) {
    let bytes = std::fs::read(path).unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    let references = file
        .sections()
        .flat_map(|section| section.relocations())
        .filter_map(|(_, relocation)| match relocation.target() {
            RelocationTarget::Symbol(index) => Some(
                file.symbol_by_index(index)
                    .unwrap()
                    .name()
                    .unwrap()
                    .to_owned(),
            ),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for name in [
        "dead_external",
        "dead_planned",
        "dead_direct",
        "dead_callee",
    ] {
        assert!(file.symbol_by_name(name).is_none(), "{name}");
        assert!(!references.contains(name), "{name}");
    }
    for name in ["live_external", "live_callee"] {
        let symbol = file.symbol_by_name(name).unwrap();
        assert!(symbol.is_undefined(), "{name}");
        assert!(symbol.is_global() && !symbol.is_weak(), "{name}");
        assert!(references.contains(name), "{name}");
        assert!(
            matches!(symbol.flags(), SymbolFlags::Elf { st_other, .. }
            if st_other & 3 == elf::STV_HIDDEN),
            "{name}"
        );
    }
    for name in ["main", "owned", "owned_odr", "projection"] {
        let symbol = file.symbol_by_name(name).unwrap();
        assert!(symbol.is_definition(), "{name}");
        assert!(
            matches!(symbol.flags(), SymbolFlags::Elf { st_other, .. }
            if st_other & 3 == elf::STV_HIDDEN),
            "{name}"
        );
    }
    assert!(file.symbol_by_name("owned_odr").unwrap().is_weak());
    assert!(file.symbol_by_name("projection").unwrap().is_weak());
    assert!(file.section_by_name(".group").is_some());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn link_and_run(directory: &Path, object: &Path) {
    use std::process::Command;

    let program = directory.join("program");
    let missing = Command::new("cc")
        .arg(object)
        .arg("-o")
        .arg(&program)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let diagnostics = String::from_utf8_lossy(&missing.stderr);
    assert!(diagnostics.contains("live_external"), "{diagnostics}");
    assert!(diagnostics.contains("live_callee"), "{diagnostics}");

    let provider = directory.join("provider.c");
    std::fs::write(
        &provider,
        "const long live_external[2] = {0, 41};\nlong live_callee(long value) { return value + 1; }\n",
    )
    .unwrap();
    let linked = Command::new("cc")
        .arg(object)
        .arg(&provider)
        .args(["-Wl,--no-undefined", "-o"])
        .arg(&program)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    assert!(Command::new(program).status().unwrap().success());
}
