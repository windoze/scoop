use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn aggregate_and_scalar_reads_share_the_machine_root() {
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
            let llvm = parse(
                &context,
                r#"
declare void @consume(ptr addrspace(1))
declare void @llvm.fake.use(...)
define void @f(ptr byval({ ptr addrspace(1) }) align 8 %source) #0 gc "statepoint-example" {
entry:
  %aggregate = load { ptr addrspace(1) }, ptr %source, align 8
  %argument = extractvalue { ptr addrspace(1) } %aggregate, 0
  %root = load ptr addrspace(1), ptr %source, align 8
  call void @consume(ptr addrspace(1) %argument) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %source, align 8
  ret void
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { noduplicate "statepoint-id"="7" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#,
            );
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let plan = crate::statepoint::rewrite(
                &llvm,
                &machine,
                &manifest(Some((7, one_root()))),
                profile,
            )
            .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("aggregate-load.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_root_counts(&path, &plan);
            assert_eq!(plan.root_count(7), Some(1), "{target:?} {mode:?}");
        }
    }
}

#[test]
fn debug_aggregate_reads_preserve_packed_offsets_alignment_and_volatile_accesses() {
    let profile = ValidatedBackendProfile::darwin_aarch64_for_test();
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = parse(
        &context,
        r#"
%Packed = type <{ i8, [2 x ptr addrspace(1)], i32 }>
define %Packed @ordinary(ptr %source) {
  %value = load %Packed, ptr %source, align 8
  ret %Packed %value
}
define %Packed @observable(ptr %source) {
  %value = load volatile %Packed, ptr %source, align 8
  ret %Packed %value
}
"#,
    );
    for name in ["ordinary", "observable"] {
        crate::statepoint::aggregate_values::normalize_loads(
            llvm.get_function(name).unwrap(),
            &machine.get_target_data(),
            profile.managed_address_space_contract(),
        )
        .unwrap();
    }
    llvm.verify().unwrap();
    let ordinary = llvm.get_function("ordinary").unwrap();
    let loads = ordinary
        .get_first_basic_block()
        .unwrap()
        .get_instructions()
        .filter(|instruction| instruction.get_opcode() == inkwell::values::InstructionOpcode::Load)
        .collect::<Vec<_>>();
    let alignments = loads
        .iter()
        .map(|load| load.get_alignment().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(alignments, [8, 1, 1, 1]);
    let ir = llvm.print_to_string().to_string();
    for offset in [1, 9, 17] {
        assert!(ir.contains(&format!("getelementptr i8, ptr %source, i64 {offset}")));
    }
    let observable = llvm.get_function("observable").unwrap();
    let accesses = observable
        .get_first_basic_block()
        .unwrap()
        .get_instructions()
        .filter(|instruction| instruction.get_opcode() == inkwell::values::InstructionOpcode::Load)
        .collect::<Vec<_>>();
    assert_eq!(accesses.len(), 1);
    assert!(accesses[0].get_volatile().unwrap());
}
