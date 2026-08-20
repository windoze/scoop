//! M0 spike: verify the inkwell -> LLVM 22.1 chain for GC statepoints
//! (stackmap emission) and landingpad-based exceptions.
//!
//! One-off code, intentionally not part of the mainline workspace.
//! See docs/ROADMAP.md, milestone M0.

use std::path::PathBuf;

use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::values::ValueKind;
use inkwell::{AddressSpace, OptimizationLevel};

fn build_module(context: &Context) -> Module<'_> {
    let module = context.create_module("scoop_m0_spike");
    let builder = context.create_builder();

    let ptr_ty = context.ptr_type(AddressSpace::default());
    let void_ty = context.void_type();
    let i32_ty = context.i32_type();

    let alloc_fn = module.add_function(
        "scoop_runtime_alloc",
        ptr_ty.fn_type(&[], false),
        Some(Linkage::External),
    );
    let may_throw_fn = module.add_function(
        "scoop_may_throw",
        void_ty.fn_type(&[], false),
        Some(Linkage::External),
    );
    let personality_fn = module.add_function(
        "__gxx_personality_v0",
        i32_ty.fn_type(&[], true),
        Some(Linkage::External),
    );

    // Function with a GC strategy: RewriteStatepointsForGC rewrites its
    // ordinary calls into statepoints. `%p` stays live across the second
    // call, so the emitted stackmap must record it as a root.
    let gc_fn = module.add_function("gc_test", ptr_ty.fn_type(&[], false), None);
    gc_fn.set_gc("statepoint-example");
    let entry = context.append_basic_block(gc_fn, "entry");
    builder.position_at_end(entry);
    let p = match builder
        .build_call(alloc_fn, &[], "p")
        .expect("call alloc")
        .try_as_basic_value()
    {
        ValueKind::Basic(value) => value,
        ValueKind::Instruction(_) => unreachable!("alloc returns a value"),
    };
    builder.build_call(may_throw_fn, &[], "").expect("call may_throw");
    builder.build_return(Some(&p)).expect("ret");

    // Function with personality + invoke + landingpad + resume.
    let eh_fn = module.add_function("eh_test", void_ty.fn_type(&[], false), None);
    eh_fn.set_personality_function(personality_fn);
    let entry = context.append_basic_block(eh_fn, "entry");
    let normal = context.append_basic_block(eh_fn, "normal");
    let unwind = context.append_basic_block(eh_fn, "unwind");
    builder.position_at_end(entry);
    builder
        .build_invoke(may_throw_fn, &[], normal, unwind, "")
        .expect("invoke");
    builder.position_at_end(normal);
    builder.build_return(None).expect("ret void");
    builder.position_at_end(unwind);
    let exception_ty = context.struct_type(&[ptr_ty.into(), i32_ty.into()], false);
    let catch_all = ptr_ty.const_null();
    let lp = builder
        .build_landing_pad(exception_ty, personality_fn, &[catch_all.into()], false, "lp")
        .expect("landingpad");
    builder.build_resume(lp).expect("resume");

    module
}

fn main() {
    Target::initialize_native(&InitializationConfig::default()).expect("native target init");

    let context = Context::create();
    let module = build_module(&context);

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).expect("host target");
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("target machine");

    module
        .run_passes(
            "rewrite-statepoints-for-gc",
            &machine,
            PassBuilderOptions::create(),
        )
        .expect("rewrite-statepoints-for-gc pass");

    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("gc.statepoint"),
        "statepoint intrinsics missing after rewrite-statepoints-for-gc:\n{ir}"
    );
    assert!(ir.contains("landingpad"), "landingpad missing from IR:\n{ir}");

    let object_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike.o");
    std::fs::create_dir_all(object_path.parent().expect("parent dir")).expect("mkdir");
    machine
        .write_to_file(&module, FileType::Object, &object_path)
        .expect("emit object file");

    let bytes = std::fs::read(&object_path).expect("read object");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("__llvm_stackmaps"),
        "object file lacks the __llvm_stackmaps section"
    );
    assert!(
        text.contains("gcc_except_tab"),
        "object file lacks exception tables"
    );

    println!("SPIKE PASS: statepoint + stackmap + landingpad verified");
    println!("object written to {}", object_path.display());
}
