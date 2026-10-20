//! Qualify mixed AS1/metadata results before changing the interface ABI.

use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

fn expected() -> ExpectedSafepoints {
    let result = CallerRootSource::Temp(la_arena::Idx::from_raw(0.into()));
    let root = |source| {
        ExpectedStatepoint::Relocating(
            vec![ExpectedRoot {
                source,
                byte_offset: 0,
            }]
            .into(),
        )
    };
    ExpectedSafepoints {
        sites: [
            (7, "f", "entry", root(CallerRootSource::Param(0))),
            (8, "f", "entry", root(result)),
            (9, "g", "entry", ExpectedStatepoint::ZeroLiveInvoke),
            (10, "g", "normal", root(result)),
            (11, "h", "merge", root(result)),
        ]
        .into_iter()
        .map(|(id, function, block, statepoint)| {
            (
                id,
                ExpectedSite {
                    function: function.into(),
                    block: block.into(),
                    statepoint,
                },
            )
        })
        .collect(),
        functions: BTreeMap::from(["f", "g", "h"].map(|name| (name.into(), GcEffect::Managed)))
            .into(),
    }
}

#[test]
fn interface_parts_call_invoke_and_relocation_survive_target_codegen() {
    let source = include_str!("interface_parts.ll");
    let selected = source.replace(
        "phi {ptr addrspace(1), ptr} [%a.complete, %left], [%b.complete, %right]",
        "select i1 %choose, {ptr addrspace(1), ptr} %a.complete, {ptr addrspace(1), ptr} %b.complete",
    );
    let bounded = source.replace(
        "  %token = invoke token",
        r#"  br label %bounded
bounded:
  %left = phi i32 [64, %entry], [%remaining, %bounded]
  %sum = phi i32 [0, %entry], [%next, %bounded]
  %scaled = mul i32 %left, 1664525
  %mixed = xor i32 %scaled, 1013904223
  %next = add i32 %sum, %mixed
  %remaining = sub i32 %left, 1
  %again = icmp ne i32 %remaining, 0
  br i1 %again, label %bounded, label %invoke.entry
invoke.entry:
  call void (...) @llvm.fake.use(i32 %next)
  %token = invoke token"#,
    );
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            for source in [source, selected.as_str(), bounded.as_str()] {
                let profile = ValidatedBackendProfile::for_selection(
                    scoop_target_data::selection(target)
                        .expect("implemented target selected by this consumer"),
                )
                .unwrap()
                .with_optimization(mode);
                let machine = profile.create_target_machine().unwrap();
                let context = Context::create();
                let llvm = parse(&context, source);
                llvm.set_triple(&machine.get_triple());
                llvm.set_data_layout(&machine.get_target_data().get_data_layout());
                let plan = crate::statepoint::rewrite(&llvm, &machine, &expected(), profile)
                    .unwrap_or_else(|error| panic!("{target:?}/{mode:?}: {error}"));

                llvm.run_passes(
                    "function(verify<safepoint-ir>),verify",
                    &machine,
                    PassBuilderOptions::create(),
                )
                .unwrap();
                for site in [7, 8, 10, 11] {
                    assert_eq!(plan.root_count(site), Some(1));
                }
                assert_eq!(plan.root_count(9), Some(0));
                let ir = llvm.print_to_string().to_string();
                assert!(ir.contains("gc.result"), "{ir}");
                assert!(ir.contains("gc.relocate.p1"), "{ir}");
                assert!(!ir.contains("gc.relocate.p0"), "{ir}");
                assert!(!ir.contains("ptrtoint"), "{ir}");
                assert!(!ir.contains("inttoptr"), "{ir}");
                assert!(
                    !machine
                        .write_to_memory_buffer(&llvm, FileType::Object)
                        .unwrap()
                        .as_slice()
                        .is_empty()
                );
            }
        }
    }
}
