use super::*;
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::values::{AnyValue, CallSiteValue};

#[test]
fn gc_leaf_bridge_preserves_side_effects_without_a_gc_boundary() {
    check_gc_leaf(false);
}

#[test]
fn gc_leaf_direct_call_preserves_side_effects_without_a_gc_boundary() {
    check_gc_leaf(true);
}

fn check_gc_leaf(direct: bool) {
    let mut module = Module {
        release_hooks: Default::default(),
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: Default::default(),
        enums: Default::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: Vec::new(),
        output: scoop_lir::LirOutput::Library,
        meta: string_metadata(),
    };
    let bridge = outbound_bridge(1);
    let native_symbol = if direct {
        "native_leaf".to_owned()
    } else {
        bridge.symbol().to_owned()
    };
    let call_plan = if direct {
        scoop_lir::CAbiCallPlan::Direct(scoop_lir::DirectCSignature {
            params: Vec::new(),
            result: scoop_lir::DirectCReturn::Void,
        })
    } else {
        scoop_lir::CAbiCallPlan::StorageBridge(Box::new(bridge))
    };
    let native = module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "leaf".into(),
            native_symbol: "native_leaf".into(),
            library: "fixture".into(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        call_mode: scoop_lir::CAbiCallMode::GcLeaf,
        call_plan,
        signature: scoop_lir::CFunctionType {
            params: Vec::new(),
            return_type: scoop_lir::CReturnType::Void,
        },
    });
    let mut targets = CallTargets::default();
    let signature = targets
        .void_signatures
        .alloc(scoop_lir::VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let target = targets.c_targets.void.alloc(scoop_lir::CallTarget {
        destination: scoop_lir::CCallDestination::extern_function(native),
        signature,
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![Instruction::Call {
            site: CallSite::NativeGcLeaf(scoop_lir::NativeGcLeafCallSite {
                call: scoop_lir::CTypedCall::Void {
                    target,
                    args: Vec::new(),
                },
            }),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let function = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: Default::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    };
    let symbol = function.symbol().to_owned();
    module.functions = vec![function];
    module.output = scoop_lir::LirOutput::Executable {
        entry: managed_function_ref(0),
    };
    install_test_native_function_contract(&mut module, 1);
    refresh_module_safepoints(&mut module);
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    statepoint::rewrite(
        &llvm,
        &machine,
        &statepoint::expectations(&module).unwrap(),
        host_profile(),
    )
    .unwrap();
    llvm.verify().unwrap();
    let function = llvm.get_function(&symbol).unwrap();
    let body = function.print_to_string().to_string();
    assert!(
        body.contains(&native_symbol),
        "the observable native call was removed: {body}"
    );
    for forbidden in ["gc.statepoint", "gc.relocate", "native_safe", "caller_root"] {
        assert!(!body.contains(forbidden), "unexpected {forbidden}: {body}");
    }
    let call = function
        .get_basic_blocks()
        .into_iter()
        .flat_map(|block| block.get_instructions())
        .filter_map(|instruction| CallSiteValue::try_from(instruction).ok())
        .find(|call| {
            call.get_called_fn_value()
                .is_some_and(|callee| callee.get_name().to_bytes() == native_symbol.as_bytes())
        })
        .unwrap();
    assert!(
        call.get_string_attribute(AttributeLoc::Function, "gc-leaf-function")
            .is_some()
    );
    for forbidden in ["memory", "nosync"] {
        assert!(
            call.get_enum_attribute(
                AttributeLoc::Function,
                Attribute::get_named_enum_kind_id(forbidden),
            )
            .is_none(),
            "unexpected {forbidden} on native call"
        );
    }
    if direct {
        assert!(
            call.get_enum_attribute(
                AttributeLoc::Function,
                Attribute::get_named_enum_kind_id("nobuiltin"),
            )
            .is_some()
        );
    }
}
