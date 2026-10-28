use super::*;

pub(in crate::tests) fn stackmap_qualification_module() -> Module {
    fn poll_function(
        symbol: &str,
        safepoint: u64,
        params: Vec<LirType>,
        return_ty: LirType,
        live: scoop_lir::StatepointLiveSet,
        return_value: Option<Value>,
    ) -> Function {
        let mut call_targets = CallTargets::default();
        let signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
        let target = call_targets
            .managed_targets
            .void
            .alloc(scoop_lir::CallTarget {
                destination: scoop_lir::ManagedCallDestination::runtime(
                    scoop_lir::ManagedRuntimeFunction::Safepoint,
                ),
                signature,
            });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            terminator_origin: Default::default(),
            name: "entry".to_string(),
            instructions: vec![Instruction::ManagedPoll {
                site: scoop_lir::ManagedPollSite {
                    kind: scoop_lir::ManagedPollKind::Full,
                    target,
                    safepoint: test_safepoint(safepoint),
                    live,
                },
            }]
            .into(),
            terminator: Terminator::Return {
                value: return_value,
            },
        });
        Function {
            debug_name: Default::default(),
            debug_origin: Default::default(),
            callable_body: callable_body(symbol),
            safepoints: test_safepoints(symbol, &blocks, entry),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(params, return_ty),
            call_targets,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks,
            entry,
        }
    }

    let pair = LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR]);
    Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![
            poll_function(
                "scoop_qualification_zero",
                1,
                Vec::new(),
                LirType::Void,
                scoop_lir::StatepointLiveSet::default(),
                None,
            ),
            poll_function(
                "scoop_qualification_one",
                2,
                vec![MANAGED_PTR],
                MANAGED_PTR,
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    MANAGED_PTR,
                    &[0],
                )]),
                Some(Value::Param(0)),
            ),
            poll_function(
                "scoop_qualification_many",
                3,
                vec![pair.clone()],
                pair.clone(),
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    pair,
                    &[0, 8],
                )]),
                Some(Value::Param(0)),
            ),
        ],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta: string_metadata(),
    }
}
