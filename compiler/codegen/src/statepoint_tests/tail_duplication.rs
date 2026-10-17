use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

const SHARED_TERMINAL_CALL: &str = r#"
declare void @left()
declare void @right()
declare void @finish()
define void @f(i1 %condition) #0 gc "statepoint-example" {
entry:
  br i1 %condition, label %a, label %b
a:
  call void @left() #1
  br label %tail
b:
  call void @right() #1
  br label %tail
tail:
  call void @finish() #2
  unreachable
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "gc-leaf-function" }
attributes #2 = { "statepoint-id"="7" }
"#;

#[test]
fn shared_terminal_call_is_not_duplicated() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, SHARED_TERMINAL_CALL);
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            crate::statepoint::configure_function(
                &context,
                llvm.get_function("f").unwrap(),
                GcEffect::Managed,
                profile,
            );
            let expected = manifest(Some((7, ExpectedStatepoint::Relocating(Vec::new().into()))));
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();

            assert_eq!(plan.root_count(7), Some(0));
            let output = directory
                .path()
                .join(format!("{}-{mode:?}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_site_count(&output, plan.site_count());
        }
    }
}
