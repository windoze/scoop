use super::*;

#[test]
fn initialization_covers_complete_reference_and_niche_stores() {
    for kind in [
        scoop_lir::NullNicheKind::Managed,
        scoop_lir::NullNicheKind::Interface,
    ] {
        for variant in [None, Some(0), Some(1)] {
            let size = if kind == scoop_lir::NullNicheKind::Interface {
                32
            } else {
                24
            };
            let (mut module, out) = fixture(size, RefScan::References(vec![16]));
            module.output = scoop_lir::LirOutput::Library;
            let wrapped = variant.map(|variant| {
                let id = module.enums.alloc(EnumDef {
                    exact_type: crate::tests::test_physical_exact(
                        "InitializationOption",
                        scoop_identity::SourceNominalKind::Enum,
                    ),
                    name: "InitializationOption".into(),
                    repr: EnumRepr::Niche {
                        kind,
                        payload_variant: 1,
                    },
                    scan: RefScan::References(vec![0]),
                });
                (id, module.enums.variant_ref(id, variant).unwrap())
            });
            let function = &mut module.functions[1];
            function.signature = plain_scoop_signature(vec![kind.storage_type()], LirType::Void);
            let value = if let Some((id, variant)) = wrapped {
                let payload = function.temps.alloc(Temp {
                    ty: LirType::Enum(id),
                });
                function.blocks[function.entry]
                    .instructions
                    .push(Instruction::EnumWrap {
                        out: payload,
                        variant,
                        fields: if variant.index() == 1 {
                            vec![Value::Param(0)]
                        } else {
                            vec![]
                        },
                    });
                Value::Temp(payload)
            } else {
                Value::Param(0)
            };
            function.blocks[function.entry]
                .instructions
                .push(Instruction::HeapStore {
                    object: Value::Temp(out),
                    offset: 16,
                    value,
                });
            check_clear(module, 16);
        }
    }
}

#[test]
fn initialization_keeps_unwritten_references_safe_at_a_poll() {
    for first_offset in [16, 24] {
        let (mut module, out) = fixture(32, RefScan::References(vec![16, 24]));
        let function = &mut module.functions[1];
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
        function.blocks[function.entry].instructions.extend([
            Instruction::HeapStore {
                object: Value::Temp(out),
                offset: first_offset,
                value: Value::Temp(out),
            },
            Instruction::ManagedPoll {
                site: scoop_lir::ManagedPollSite {
                    kind: scoop_lir::ManagedPollKind::Full,
                    target,
                    safepoint: test_safepoint(105),
                    live: statepoint_live(vec![statepoint_value(
                        scoop_lir::CallerRootSource::Temp(out),
                        MANAGED_PTR,
                        &[0],
                    )]),
                },
            },
            Instruction::HeapStore {
                object: Value::Temp(out),
                offset: 40 - first_offset,
                value: Value::Temp(out),
            },
        ]);
        check_clear(module, if first_offset == 24 { 24 } else { 32 });
    }
}

#[test]
fn initialization_preserves_tagged_enum_storage() {
    let (mut module, out) = fixture(48, RefScan::References(vec![40]));
    module.output = scoop_lir::LirOutput::Library;
    module.enums = crate::tests::enums::enum_module().enums;
    let id = module
        .enums
        .iter()
        .find(|(_, def)| matches!(def.repr, EnumRepr::Tagged { .. }))
        .unwrap()
        .0;
    let function = &mut module.functions[1];
    function.signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![LirType::Enum(id)],
        LirType::Void,
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::HeapStore {
            object: Value::Temp(out),
            offset: 16,
            value: Value::Param(0),
        });
    check_clear(module, 48);
}

#[test]
fn initialization_covers_dense_reference_structs_and_tuples() {
    for named in [false, true] {
        let (mut module, out) = fixture(32, RefScan::References(vec![16]));
        module.output = scoop_lir::LirOutput::Library;
        let fields = vec![MANAGED_PTR, LirType::I64];
        let ty = if named {
            let reference = module.structs.alloc_scoop(
                test_exact_type("ReferenceInitializationPair"),
                "ReferenceInitializationPair".into(),
                16,
                8,
                false,
                fields
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| scoop_lir::StructField {
                        ty: ty.clone(),
                        layout: scoop_lir::FieldLayout {
                            offset: index as u64 * 8,
                            access_align: 8,
                        },
                    })
                    .collect(),
            );
            LirType::Struct(reference.definition())
        } else {
            LirType::Aggregate(fields)
        };
        let function = &mut module.functions[1];
        function.signature =
            scoop_signature(&module.structs, &module.enums, vec![ty], LirType::Void);
        function.blocks[function.entry]
            .instructions
            .push(Instruction::HeapStore {
                object: Value::Temp(out),
                offset: 16,
                value: Value::Param(0),
            });
        check_clear(module, 16);
    }
}
