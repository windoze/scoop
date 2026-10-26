use super::*;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::passes::PassBuilderOptions;

fn profiles() -> [ValidatedBackendProfile; 2] {
    [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
    ]
    .map(|selection| {
        ValidatedBackendProfile::for_selection(selection)
            .unwrap()
            .with_optimization(scoop_lir::OptimizationMode::Release)
    })
}

#[test]
fn raw_storage_retains_exact_extent_alignment_and_managed_offsets() {
    let context = Context::create();
    for profile in profiles() {
        let machine = profile.create_target_machine().unwrap();
        let layout = machine.get_target_data();
        for (size, align, references) in [
            (0, 1, vec![]),
            (3, 1, vec![]),
            (6, 2, vec![]),
            (12, 4, vec![]),
            (24, 8, vec![]),
            (24, 8, vec![0, 16]),
            (48, 16, vec![8, 24, 40]),
        ] {
            let storage = storage_type(
                &context,
                profile.managed_address_space_contract(),
                size,
                align,
                &RefScan::References(references.clone()),
            )
            .unwrap();
            assert_eq!(layout.get_store_size(&storage), size);
            assert_eq!(u64::from(layout.get_abi_alignment(&storage)), align);
            let actual = storage
                .get_field_types()
                .iter()
                .enumerate()
                .filter_map(|(index, field)| {
                    if let BasicTypeEnum::PointerType(pointer) = field {
                        assert_eq!(pointer.get_address_space(), AddressSpace::from(1));
                        Some(layout.offset_of_element(&storage, index as u32).unwrap())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, references);
        }
    }
}

#[test]
fn wrapper_to_scalar_copy_vectorizes_without_byte_gathers() {
    let context = Context::create();
    for profile in profiles() {
        let machine = profile.create_target_machine().unwrap();
        let storage = storage_type(
            &context,
            profile.managed_address_space_contract(),
            8,
            8,
            &RefScan::None,
        )
        .unwrap();
        let source = format!(
            r#"
define void @copy(ptr %source, ptr %target) {{
entry:
  %slot = alloca i64, align 8
  br label %loop
loop:
  %index = phi i64 [0, %entry], [%next, %loop]
  %input = getelementptr i64, ptr %source, i64 %index
  %wrapper = load {storage}, ptr %input, align 8
  store {storage} %wrapper, ptr %slot, align 8
  %value = load i64, ptr %slot, align 8
  %output = getelementptr i64, ptr %target, i64 %index
  store i64 %value, ptr %output, align 8
  %next = add nuw nsw i64 %index, 1
  %again = icmp ult i64 %next, 64
  br i1 %again, label %loop, label %exit, !llvm.loop !0
exit:
  ret void
}}
!0 = distinct !{{!0, !1}}
!1 = !{{!"llvm.loop.vectorize.enable", i1 true}}
"#,
            storage = storage.print_to_string().to_str().unwrap()
        );
        let buffer = MemoryBuffer::create_from_memory_range_copy(source.as_bytes(), "copy");
        let module = context.create_module_from_ir(buffer).unwrap();
        module.set_data_layout(&machine.get_target_data().get_data_layout());
        module.set_triple(&machine.get_triple());
        module
            .run_passes(
                "sroa,mem2reg,instcombine<no-verify-fixpoint>,loop-simplify,lcssa,loop-vectorize,instcombine<no-verify-fixpoint>",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
        module.verify().unwrap();
        let ir = module.print_to_string().to_string();
        assert!(ir.contains("load <2 x i64>"), "{ir}");
        assert!(!ir.contains("load i8") && !ir.contains(" x i8>"), "{ir}");
    }
}
