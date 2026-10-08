//! LLVM 22.1 feasibility check for the M33 managed-reference atomics.

use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedLirTargetSelection};

const SOURCE: &str = include_str!("atomic_refs.ll");

fn operations() -> Vec<(&'static str, String)> {
    let mut operations = Vec::new();
    for order in ["monotonic", "acquire", "seq_cst"] {
        operations.push((
            "load atomic",
            format!("%observed = load atomic ptr addrspace(1), ptr addrspace(1) %address {order}, align 8"),
        ));
    }
    for order in ["monotonic", "release", "seq_cst"] {
        operations.push((
            "store atomic",
            format!(
                "store atomic ptr addrspace(1) %value, ptr addrspace(1) %address {order}, align 8"
            ),
        ));
    }
    for order in ["monotonic", "acquire", "release", "acq_rel", "seq_cst"] {
        operations.push((
            "atomicrmw xchg",
            format!("%observed = atomicrmw xchg ptr addrspace(1) %address, ptr addrspace(1) %value {order}, align 8"),
        ));
    }
    for (success, failures) in [
        ("monotonic", &["monotonic"][..]),
        ("acquire", &["monotonic", "acquire"]),
        ("release", &["monotonic"]),
        ("acq_rel", &["monotonic", "acquire"]),
        ("seq_cst", &["monotonic", "acquire", "seq_cst"]),
    ] {
        for failure in failures {
            operations.push((
                "cmpxchg",
                format!("%pair = cmpxchg ptr addrspace(1) %address, ptr addrspace(1) %expected, ptr addrspace(1) %value {success} {failure}, align 8\n  %observed = extractvalue {{ptr addrspace(1), i1}} %pair, 0"),
            ));
        }
    }
    operations
}

fn expected() -> ExpectedSafepoints {
    let parameters = (0..3)
        .map(|index| ExpectedRoot {
            source: CallerRootSource::Param(index),
            byte_offset: 0,
        })
        .collect::<Vec<_>>();
    let mut result = parameters.clone();
    result.push(ExpectedRoot {
        source: CallerRootSource::Local(la_arena::Idx::from_raw(0.into())),
        byte_offset: 0,
    });
    ExpectedSafepoints {
        sites: [(7, parameters), (8, result)]
            .into_iter()
            .map(|(id, roots)| {
                (
                    id,
                    ExpectedSite {
                        function: "f".into(),
                        block: "entry".into(),
                        statepoint: ExpectedStatepoint::Relocating(roots.into()),
                    },
                )
            })
            .collect(),
        functions: BTreeMap::from([("f".into(), GcEffect::Managed)]),
    }
}

#[test]
fn atomic_managed_references_survive_rewrite_and_target_codegen() {
    let operations = operations();
    assert_eq!(operations.len(), 20);
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::from_selection(
                ValidatedLirTargetSelection::from_id(target),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            for (opcode, operation) in &operations {
                let context = Context::create();
                let source = SOURCE.replace("ATOMIC_OPERATION", operation).replace(
                    "OBSERVED_VALUE",
                    if *opcode == "store atomic" {
                        "%value"
                    } else {
                        "%observed"
                    },
                );
                let llvm = parse(&context, &source);
                llvm.set_triple(&machine.get_triple());
                llvm.set_data_layout(&machine.get_target_data().get_data_layout());
                let plan = crate::statepoint::rewrite(&llvm, &machine, &expected(), profile)
                    .unwrap_or_else(|error| panic!("{target:?}/{mode:?}/{operation}: {error}"));
                verify_rewritten_with_profile(&llvm, &plan, profile)
                    .unwrap_or_else(|error| panic!("{target:?}/{mode:?}/{operation}: {error}"));
                llvm.run_passes(
                    "function(verify<safepoint-ir>),verify",
                    &machine,
                    PassBuilderOptions::create(),
                )
                .unwrap();
                assert_eq!(plan.root_count(7), Some(3));
                assert_eq!(
                    plan.root_count(8),
                    Some(if *opcode == "store atomic" { 3 } else { 4 })
                );
                let ir = llvm.print_to_string().to_string();
                assert!(ir.contains(opcode), "{target:?}/{mode:?}: {ir}");
                assert!(!ir.contains("cmpxchg weak"), "{ir}");
                assert!(!ir.contains("ptrtoint"), "{ir}");
                assert!(!ir.contains("inttoptr"), "{ir}");
                let object = machine
                    .write_to_memory_buffer(&llvm, FileType::Object)
                    .unwrap();
                assert!(!object.as_slice().is_empty());
            }
        }
    }
}
