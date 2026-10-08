use super::*;
use inkwell::targets::FileType;
use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedLirTargetSelection};

const SOURCE: &str = r#"
declare void @tick()
declare void @callee(ptr byval(i32) align 4)
declare void @llvm.fake.use(...)
define void @f(i32 %value, ptr addrspace(1) %root) #0 gc "statepoint-example" {
entry:
  %argument = alloca i32, align 4
  store i32 %value, ptr %argument
  call void @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  call void @callee(ptr byval(i32) align 4 %argument) #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !1
  ret void
}
attributes #0 = { noredzone "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
!0 = !{i64 7, i64 0, i64 1, i64 0}
!1 = !{i64 8, i64 0, i64 1, i64 0}
"#;

#[test]
fn amd64_byval_calls_preserve_the_fixed_managed_frame() {
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let profile =
            ValidatedBackendProfile::from_selection(ValidatedLirTargetSelection::from_id(target))
                .unwrap()
                .with_optimization(OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, SOURCE);
        let expected = ExpectedSafepoints {
            sites: [7, 8]
                .into_iter()
                .map(|id| {
                    (
                        id,
                        ExpectedSite {
                            function: "f".into(),
                            block: "entry".into(),
                            statepoint: ExpectedStatepoint::Relocating(
                                vec![ExpectedRoot {
                                    source: CallerRootSource::Param(1),
                                    byte_offset: 0,
                                }]
                                .into(),
                            ),
                        },
                    )
                })
                .collect(),
            functions: BTreeMap::from([("f".into(), GcEffect::Managed)]),
        };
        let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        verify_rewritten_with_profile(&llvm, &plan, profile).unwrap();
        let assembly = machine
            .write_to_memory_buffer(&llvm, FileType::Assembly)
            .unwrap();
        let assembly = std::str::from_utf8(assembly.as_slice()).unwrap();
        let start = assembly.find("callq\ttick").expect(assembly);
        let end = assembly.find("callq\tcallee").expect(assembly);
        let calls = &assembly[start..end];
        assert!(
            !calls.lines().any(|line| {
                line.contains("pushq") || (line.contains("subq") && line.contains("%rsp"))
            }),
            "{target:?}: byval arguments changed the call-site SP:\n{assembly}"
        );
    }
}
