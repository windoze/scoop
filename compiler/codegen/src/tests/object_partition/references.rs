use super::*;

#[test]
fn callable_modules_declare_their_own_data_and_keep_complete_metadata_definitions() {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    let ids = module.globals.iter().map(|(id, _)| id).collect::<Vec<_>>();
    let unused = module.globals.alloc(Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: immortal_string_identity("unused-data"),
            value: "unused data remains in the metadata partition".into(),
        },
    });
    let symbols = [ids[0], ids[1], unused].map(|id| module.globals[id].symbol().to_owned());
    module.functions = ids
        .iter()
        .enumerate()
        .map(|(ordinal, &global)| {
            let mut blocks = Arena::default();
            let entry = blocks.alloc(BasicBlock {
                name: "entry".into(),
                instructions: Vec::new(),
                terminator: Terminator::Return {
                    value: Some(Value::Global(global)),
                },
            });
            Function {
                callable_body: callable_body(&format!("selectData{ordinal}")),
                gc_effect: GcEffect::NoGc,
                signature: plain_scoop_signature(Vec::new(), MANAGED_PTR),
                call_targets: CallTargets::default(),
                safepoints: scoop_lir::SafepointIdentities::default(),
                locals: Arena::default(),
                temps: Arena::default(),
                blocks,
                entry,
            }
        })
        .collect();
    let bodies = module
        .functions
        .iter()
        .map(|function| function.callable_body.id())
        .collect::<Vec<_>>();
    let descriptors = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor.identity.symbol().to_owned())
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

    for (ordinal, body) in bodies.iter().enumerate() {
        let member = rendered
            .iter()
            .find(|member| member.units().kind() == ScoopLirObjectKindV1::CallableBody(*body))
            .unwrap();
        assert!(member.llvm_ir().contains(&symbols[ordinal]));
        for (other, symbol) in symbols.iter().enumerate() {
            if other != ordinal {
                assert!(
                    !member.llvm_ir().contains(symbol),
                    "unrelated global declared by callable"
                );
            }
        }
        for descriptor in &descriptors {
            assert!(
                !member.llvm_ir().contains(descriptor),
                "unreferenced descriptor declared by callable"
            );
        }
    }
    for symbol in &symbols {
        assert!(rendered.iter().any(|member| member.units().kind()
            == ScoopLirObjectKindV1::NonCallable
            && member.llvm_ir().contains(symbol)));
    }
}
