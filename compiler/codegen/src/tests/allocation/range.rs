use super::*;

#[test]
fn constant_allocations_use_the_selected_target_range_check() {
    for size in [32, 128, 136] {
        let ir = allocation_ir(size, false, true, scoop_lir::OptimizationMode::Release);
        if host_target_machine().unwrap().get_target().get_name() == c"aarch64" {
            assert!(ir.contains("tlab_remaining = sub i64"), "{ir}");
            assert!(
                ir.contains(&format!("icmp ule i64 {size}, %tlab_remaining")),
                "{ir}"
            );
            assert!(!ir.contains("tlab_present"), "{ir}");
            assert!(!ir.contains("tlab_cursor_plus_align"), "{ir}");
        } else {
            assert!(!ir.contains("tlab_remaining"), "{ir}");
            assert!(ir.contains("tlab_present"), "{ir}");
            assert!(ir.contains("tlab_cursor_plus_align"), "{ir}");
        }
        assert!(ir.contains("@scoop_runtime_alloc_slow"), "{ir}");
    }
}

#[test]
fn general_allocations_keep_alignment_presence_and_limit_checks() {
    for (dynamic, known, mode) in [
        (false, true, scoop_lir::OptimizationMode::Debug),
        (true, true, scoop_lir::OptimizationMode::Release),
        (false, false, scoop_lir::OptimizationMode::Release),
    ] {
        let ir = allocation_ir(32, dynamic, known, mode);
        assert!(!ir.contains("tlab_remaining"), "{ir}");
        assert!(ir.contains("tlab_present"), "{ir}");
        assert!(ir.contains("tlab_cursor_plus_align"), "{ir}");
        assert!(ir.contains("alloc_within_tlab = icmp ule"), "{ir}");
    }

    let (mut module, _) = initialization::fixture(32, RefScan::None);
    let descriptor = module
        .meta
        .type_descriptors
        .iter_mut()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "Point")
        .unwrap()
        .1;
    descriptor.instance_shape =
        TypeInstanceShapeV1::fixed_object(module.meta.target_profile, 32, 16, RefScan::None)
            .unwrap();
    refresh_module_safepoints(&mut module);
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let llvm = emit_llvm_module(
        &context,
        &module,
        &machine,
        host_profile().with_optimization(scoop_lir::OptimizationMode::Release),
    )
    .unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    assert!(!ir.contains("tlab_remaining"), "{ir}");
    assert!(ir.contains("tlab_present"), "{ir}");
    assert!(ir.contains("tlab_cursor_plus_align"), "{ir}");
}
