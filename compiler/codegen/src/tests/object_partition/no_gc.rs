use super::*;
use object::ObjectSection;

fn empty_leaf(name: &str) -> Function {
    let empty = LirType::Aggregate(Vec::new());
    let mut temps = Arena::new();
    let result = temps.alloc(Temp { ty: empty.clone() });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: vec![Instruction::MakeAggregate {
            out: result,
            elements: Vec::new(),
        }]
        .into(),
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_origin: Default::default(),
        callable_body: callable_body(name),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(Vec::new(), empty),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals: Arena::new(),
        temps,
        blocks,
        entry,
    }
}

#[test]
fn no_gc_calls_with_elided_results_keep_the_planned_compact_unwind_atoms() {
    let mut module = values_module();
    let leaf = empty_leaf("emptyLeaf");
    let mut caller = empty_leaf("emptyPairSecondary");
    let mut calls = Vec::new();
    for _ in 0..2 {
        let result = caller.temps.alloc(Temp {
            ty: LirType::Aggregate(Vec::new()),
        });
        let site = elided_zst_site(
            &mut caller.call_targets,
            TestCallProtocol::NoGc {
                destination: no_gc_local(0),
            },
            Vec::new(),
            LirType::Aggregate(Vec::new()),
            result,
            Vec::new(),
        );
        calls.push(Instruction::Call { site });
    }
    caller.blocks[caller.entry].instructions.splice(0..0, calls);
    module.functions = vec![leaf, caller];
    module.output = scoop_lir::LirOutput::Library;
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        directory.path(),
        host_profile(),
    )
    .unwrap();
    let mut count = 0;
    for member in emitted.members() {
        if !matches!(
            member.kind(),
            EmittedConeObjectMemberKindV1::CallableBody { .. }
        ) {
            continue;
        }
        count += 1;
        let bytes = std::fs::read(member.path()).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        assert!(object.section_by_name("__eh_frame").is_none());
        assert!(object.section_by_name("__llvm_stackmaps").is_none());
        assert!(object.section_by_name("__compact_unwind").unwrap().size() > 0);
        let plan = member
            .units()
            .definition_plans()
            .iter()
            .filter_map(|id| emitted.production().canonical_definitions().plan(*id))
            .find(|plan| plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody)
            .unwrap();
        let roles = plan
            .atom_boundaries()
            .iter()
            .map(|boundary| boundary.atom_role())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            roles,
            BTreeSet::from([
                scoop_lir::DefinitionAtomRole::Primary,
                scoop_lir::DefinitionAtomRole::CompactUnwind
            ])
        );
    }
    assert_eq!(count, 2);
}

#[test]
fn callable_modules_declare_only_their_targets_and_function_addresses() {
    let mut module = values_module();
    module.functions = (0..12)
        .map(|index| empty_leaf(&format!("selected{index}")))
        .collect();
    module.output = scoop_lir::LirOutput::Library;
    let caller = &mut module.functions[0];
    for index in [0, 1, 1] {
        let result = caller.temps.alloc(Temp {
            ty: LirType::Aggregate(Vec::new()),
        });
        let site = elided_zst_site(
            &mut caller.call_targets,
            TestCallProtocol::NoGc {
                destination: no_gc_local(index),
            },
            Vec::new(),
            LirType::Aggregate(Vec::new()),
            result,
            Vec::new(),
        );
        caller.blocks[caller.entry]
            .instructions
            .push(Instruction::Call { site });
    }
    let address = caller.temps.alloc(Temp {
        ty: scoop_lir::CODE_PTR,
    });
    caller.blocks[caller.entry]
        .instructions
        .push(Instruction::FunctionAddress {
            out: address,
            target: scoop_lir::FunctionAddressTarget::Local(local_function_ref(2, GcEffect::NoGc)),
        });
    let symbols = module
        .functions
        .iter()
        .map(|function| (function.callable_body.id(), function.symbol().to_string()))
        .collect::<Vec<_>>();
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let rendered = render_llvm_ir_members(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        host_profile(),
    )
    .unwrap();
    for member in rendered {
        let ScoopLirObjectKindV1::CallableBody(body) = member.units().kind() else {
            continue;
        };
        let actual = symbols
            .iter()
            .filter(|(_, symbol)| {
                member.llvm_ir().lines().any(|line| {
                    (line.starts_with("define ") || line.starts_with("declare "))
                        && line.contains(&format!("@\"{symbol}\""))
                })
            })
            .map(|(body, _)| *body)
            .collect::<BTreeSet<_>>();
        let expected = if body == symbols[0].0 {
            symbols[..3].iter().map(|(body, _)| *body).collect()
        } else {
            BTreeSet::from([body])
        };
        assert_eq!(actual, expected);
        assert!(
            !member
                .llvm_ir()
                .contains(&format!("@\"{}.1\"", symbols[0].1))
        );
    }
}
