use super::support::*;
use super::*;
use crate::*;

fn invoke(shuffled: bool) -> Module {
    let mut module = roots(shuffled);
    let function = &mut module.functions[0];
    let entry = function.entry;
    let Instruction::ManagedPoll { site } = function.blocks[entry].instructions.remove(2) else {
        panic!("poll instruction")
    };
    let comparison = function.blocks[entry].instructions.pop().unwrap();
    let terminator = std::mem::replace(
        &mut function.blocks[entry].terminator,
        Terminator::Unreachable,
    );
    let normal = function.blocks.alloc(BasicBlock {
        name: "normal".to_string(),
        instructions: vec![comparison],
        terminator,
    });
    let record = function.temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let raw = function.temps.alloc(Temp { ty: RAW_PTR });
    let unwind = function.blocks.alloc(BasicBlock {
        name: "unwind".to_string(),
        instructions: vec![Instruction::CleanupPad { record, raw }],
        terminator: Terminator::Resume {
            exception: Value::Temp(record),
        },
    });
    let roots = ExceptionalRootSet::new(
        site.live
            .as_slice()
            .iter()
            .map(|value| ExceptionalRoot {
                root: CallerRoot {
                    source: value.source,
                    scan: NonEmptyRefScan::new(RefScan::References(vec![0])).unwrap(),
                },
                normal_live: true,
                unwind_live: false,
            })
            .collect(),
    );
    function.safepoints = SafepointIdentities::checked(vec![(
        site.safepoint,
        SafepointIdentity::new(
            function.callable_body.id(),
            SafepointSiteRole::ManagedInvoke,
            0,
        )
        .unwrap(),
    )])
    .unwrap();
    function.blocks[entry]
        .instructions
        .push(Instruction::Invoke {
            site: InvokeSite::Managed(ManagedInvokeSite {
                call: ManagedTypedCall::Void {
                    target: site.target,
                    args: Vec::new(),
                },
                safepoint: site.safepoint,
                roots,
                normal,
                unwind,
            }),
        });
    function.blocks[entry].terminator = Terminator::Br(normal);
    module
}

fn fingerprint(module: &Module) -> Digest256 {
    canonical_callable_lir_fingerprint(module, &module.functions[0]).unwrap()
}

#[test]
fn canonical_invoke_keeps_exceptional_liveness_and_unwind_body() {
    let original = invoke(false);
    let expected = fingerprint(&original);
    let mut changed = invoke(true);
    assert_eq!(expected, fingerprint(&changed));
    let function = &mut changed.functions[0];
    let Instruction::Invoke {
        site: InvokeSite::Managed(site),
    } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("managed invoke")
    };
    let mut roots = site.roots.as_slice().to_vec();
    roots[0].unwind_live = true;
    site.roots = ExceptionalRootSet::new(roots);
    assert_ne!(expected, fingerprint(&changed));

    let mut changed = invoke(false);
    let function = &mut changed.functions[0];
    let Instruction::Invoke { site } = &function.blocks[function.entry].instructions[2] else {
        panic!("invoke")
    };
    let unwind = site.unwind();
    let Instruction::CleanupPad { record, raw } = function.blocks[unwind].instructions[0] else {
        panic!("cleanup")
    };
    let caught = function.temps.alloc(Temp { ty: MANAGED_PTR });
    function.blocks[unwind].instructions = vec![
        Instruction::LandingPad { record, raw },
        Instruction::BeginCatch {
            out: caught,
            raw: Value::Temp(raw),
        },
        Instruction::EndCatch,
    ];
    function.blocks[unwind].terminator = Terminator::Return {
        value: Some(Value::BoolConst(false)),
    };
    assert_ne!(expected, fingerprint(&changed));
}
