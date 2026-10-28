use super::variables::{bind, origin};
use super::*;
use scoop_lir::{DebugStorage, DebugType, DebugTypeKind, DebugTypeRef};

#[test]
fn managed_observations_end_at_safepoints_without_adding_roots() {
    let mut module = values_module();
    let exact = test_exact_type("debug.Object");
    module.meta.debug_types.insert(
        exact,
        DebugType {
            name: "Object".into(),
            size: 8,
            align: 8,
            kind: DebugTypeKind::Pointer(DebugTypeRef::Inline(Box::new(DebugType {
                name: "ObjectHeader".into(),
                size: 16,
                align: 8,
                kind: DebugTypeKind::Record(Vec::new()),
            }))),
        },
    );
    let function = &mut module.functions[0];
    function.debug_origin = origin(1);
    function.signature = plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR);
    function.locals = Arena::default();
    function.temps = Arena::default();
    function.call_targets = CallTargets::default();
    let dead = function.locals.alloc(test_local("dead", MANAGED_PTR));
    bind(function, "live", exact, DebugStorage::Parameter(0), Some(1));
    bind(function, "dead", exact, DebugStorage::Local(dead), None);
    let signature = function
        .call_targets
        .void_signatures
        .alloc(VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let target = function
        .call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature,
        });
    let live = scoop_lir::StatepointLiveSet::new(vec![scoop_lir::StatepointLiveValue {
        restore: true,
        source: scoop_lir::CallerRootSource::Param(0),
        ty: MANAGED_PTR,
        leaves: scoop_lir::ManagedLeafPaths::new(vec![scoop_lir::ManagedLeafPath {
            byte_offset: 0,
        }])
        .unwrap(),
    }])
    .unwrap();
    function.blocks = Arena::default();
    function.entry = function.blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![
            Instruction::Store {
                local: dead,
                value: Value::Param(0),
            },
            Instruction::Call {
                site: CallSite::Managed(scoop_lir::ManagedCallSite {
                    call: TypedCall::Void {
                        target,
                        args: Vec::new(),
                    },
                    safepoint: test_safepoint(1),
                    live,
                }),
            },
        ]
        .into(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
        terminator_origin: origin(8),
    });
    function.safepoints = test_safepoints("scoop_main", &function.blocks, function.entry);
    let mut plain_counts = None;
    for level in [
        scoop_lir::DebugInfoLevel::None,
        scoop_lir::DebugInfoLevel::Full,
    ] {
        let profile = host_profile().with_debug_info(level);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
        llvm.verify().unwrap();
        let before = llvm.print_to_string().to_string();
        if level == scoop_lir::DebugInfoLevel::Full {
            assert!(
                !before.contains("#dbg_declare"),
                "managed locations must be invalidatable: {before}"
            );
            let dead_id = variable_id(&before, "dead");
            let dead_records = observations(&before, dead_id);
            assert!(dead_records.len() >= 2, "{before}");
            assert!(
                dead_records.last().unwrap().contains("i8 undef"),
                "{before}"
            );
            let live_id = variable_id(&before, "live");
            let live_records = observations(&before, live_id);
            assert!(
                live_records.iter().any(|line| line.contains("i8 undef")),
                "{before}"
            );
            assert!(
                !live_records.last().unwrap().contains("undef"),
                "an actual reload restores the observation: {before}"
            );
        }
        let expected = statepoint::expectations(&module).unwrap();
        statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        llvm.verify().unwrap();
        let after = llvm.print_to_string().to_string();
        let counts = [
            " alloca ",
            "call token (i64",
            "@llvm.experimental.gc.relocate",
        ]
        .map(|needle| after.matches(needle).count());
        assert!(counts[2] > 0, "the live value must be relocated: {after}");
        if let Some(plain) = plain_counts {
            assert_eq!(
                counts, plain,
                "debug metadata must not create roots, slots or statepoints"
            );
            let dead_records = observations(&after, variable_id(&after, "dead"));
            assert!(
                dead_records.last().unwrap().contains("undef"),
                "a stale observation survived: {after}"
            );
        } else {
            plain_counts = Some(counts);
        }
    }
}

fn variable_id<'a>(ir: &'a str, name: &str) -> &'a str {
    ir.lines()
        .find(|line| line.contains(&format!("!DILocalVariable(name: \"{name}\"")))
        .unwrap()
        .split_once(" = ")
        .unwrap()
        .0
}

fn observations<'a>(ir: &'a str, id: &str) -> Vec<&'a str> {
    ir.lines()
        .filter(|line| line.contains("#dbg_value(") && line.contains(&format!(", {id},")))
        .collect()
}
