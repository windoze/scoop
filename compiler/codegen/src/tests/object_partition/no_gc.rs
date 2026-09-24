use super::*;
use object::ObjectSection;

fn empty_leaf(name: &str) -> Function {
    let empty = LirType::Aggregate(Vec::new());
    let mut temps = Arena::new();
    let result = temps.alloc(Temp { ty: empty.clone() });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::MakeAggregate {
            out: result,
            elements: Vec::new(),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    Function {
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
    let input = scoop_lir::SingleConeStrongLirOutput::try_new(module, Vec::new(), None).unwrap();
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
            EmittedStrongObjectMemberKindV1::CallableBody { .. }
        ) {
            continue;
        }
        count += 1;
        let bytes = std::fs::read(member.path()).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        assert!(object.section_by_name("__eh_frame").is_none());
        assert!(object.section_by_name("__llvm_stackmaps").is_none());
        assert!(object.section_by_name("__compact_unwind").unwrap().size() > 0);
        let [definition] = member.units().definition_plans() else {
            panic!("one callable owns each object")
        };
        let plan = emitted
            .production()
            .canonical_definitions()
            .plan(*definition)
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
