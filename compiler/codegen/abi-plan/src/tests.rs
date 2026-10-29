use super::*;
use lir::*;

fn storage(ty: LirType, size: u64) -> AbiValue {
    AbiValue::new(ty, AbiNonZeroLayout::new(size, 8).unwrap(), RefScan::None).unwrap()
}

fn input(target: LirTargetProfile) -> AbiPlanningInput {
    AbiPlanningInput {
        target,
        values: Default::default(),
        scoop: Default::default(),
        c: Default::default(),
    }
}

fn aggregate(input: &mut AbiPlanningInput, fields: Vec<LirType>) -> AbiPlanningValueId {
    let size = fields.len() as u64 * 8;
    let planned_fields = fields
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let value = input.values.alloc(AbiPlanningValue::Scalar {
                value: storage(ty.clone(), 8),
                carrier: if *ty == LirType::F64 {
                    AbiCarrier::Float(FloatKind::F64)
                } else {
                    AbiCarrier::Integer(64)
                },
            });
            AbiAggregateField {
                offset: i as u64 * 8,
                value,
            }
        })
        .collect();
    input.values.alloc(AbiPlanningValue::Aggregate {
        value: storage(LirType::Aggregate(fields), size),
        fields: planned_fields,
    })
}

#[test]
fn whole_signature_accounts_for_sret_and_keeps_spilled_sse_registers_available() {
    for target in [
        scoop_target_data::LINUX_X86_64_GNU,
        scoop_target_data::LINUX_X86_64_MUSL,
    ] {
        let mut input = input(target);
        let integer = input.values.alloc(AbiPlanningValue::Scalar {
            value: storage(LirType::I64, 8),
            carrier: AbiCarrier::Integer(64),
        });
        let mixed = aggregate(&mut input, vec![LirType::F64, LirType::I64]);
        let doubles = aggregate(&mut input, vec![LirType::F64, LirType::F64]);
        let large = aggregate(&mut input, vec![LirType::I64; 3]);
        let mut arguments = vec![integer; 5];
        arguments.push(mixed);
        arguments.extend([doubles; 5]);
        let void = input.scoop.alloc(ScoopAbiRequest {
            arguments: arguments.clone(),
            result: LogicalAbiResult::Void,
        });
        let sret = input.scoop.alloc(ScoopAbiRequest {
            arguments,
            result: LogicalAbiResult::Value(large),
        });
        let plans = plan(&input).unwrap();
        assert!(matches!(
            plans.scoop(void).arguments()[5],
            AbiArgument::Direct(_)
        ));
        let signature = plans.scoop(sret);
        assert!(matches!(signature.result(), AbiReturn::Indirect(_)));
        assert!(matches!(signature.arguments()[5], AbiArgument::Indirect(_)));
        assert!(
            signature.arguments()[6..10]
                .iter()
                .all(|arg| matches!(arg, AbiArgument::Direct(_)))
        );
        assert!(matches!(
            signature.arguments()[10],
            AbiArgument::Indirect(_)
        ));
    }
}

#[test]
fn batch_requests_preserve_zst_and_scanned_storage_without_leaf_classification() {
    let mut input = input(scoop_target_data::DARWIN_AARCH64);
    let empty = input.values.alloc(AbiPlanningValue::ZeroSized(
        AbiZst::new(
            LirType::Aggregate(vec![]),
            AbiZeroSizedLayout::new(1).unwrap(),
        )
        .unwrap(),
    ));
    let scanned = input.values.alloc(AbiPlanningValue::ScannedAggregate(
        AbiValue::new(
            LirType::Aggregate(vec![MANAGED_PTR, LirType::I64]),
            AbiNonZeroLayout::new(16, 8).unwrap(),
            RefScan::References(vec![0]),
        )
        .unwrap(),
    ));
    let request = input.scoop.alloc(ScoopAbiRequest {
        arguments: vec![empty, scanned],
        result: LogicalAbiResult::Value(empty),
    });
    let plans = plan(&input).unwrap();
    let signature = plans.scoop(request);
    assert!(matches!(
        signature.arguments()[0],
        AbiArgument::ElidedZst(_)
    ));
    assert!(matches!(signature.arguments()[1], AbiArgument::Indirect(_)));
    assert!(matches!(signature.result(), AbiReturn::ElidedZst(_)));
}

#[test]
fn canonical_unknown_target_cannot_fall_back_to_a_procedure_abi() {
    const UNKNOWN: scoop_lir::TargetProfileId =
        match scoop_lir::TargetProfileId::new("unknown-target", 1) {
            Ok(id) => id,
            Err(_) => panic!("canonical test identity"),
        };
    let target =
        scoop_lir::LirTargetProfile::new(&UNKNOWN, scoop_target_data::DARWIN_AARCH64.description());
    assert!(
        matches!(plan(&input(target)), Err(AbiPlanError::UnsupportedTarget(id)) if id == UNKNOWN)
    );
}

#[test]
fn win64_interface_result_is_scanned_sret_and_arguments_remain_two_pointers() {
    let mut input = input(scoop_target_data::DARWIN_AARCH64);
    let value = AbiValue::new(
        LirType::Interface,
        AbiNonZeroLayout::new(16, 8).unwrap(),
        RefScan::References(vec![0]),
    )
    .unwrap();
    let interface = input
        .values
        .alloc(AbiPlanningValue::Interface(value.clone()));
    let signature = scoop::classify(
        &input,
        &ScoopAbiRequest {
            arguments: vec![interface; 3],
            result: LogicalAbiResult::Value(interface),
        },
        ProcedureAbi::Win64,
    )
    .unwrap();
    assert_eq!(signature.result(), &AbiReturn::Indirect(value));
    assert_eq!(
        signature.result().scan(),
        Some(&RefScan::References(vec![0]))
    );
    assert_eq!(signature.physical_parameter_count(), 7);
    for argument in signature.arguments() {
        let AbiArgument::Direct(AbiDirectValue::DirectParts(parts)) = argument else {
            panic!("interface arguments keep their explicit pointer carriers");
        };
        assert_eq!(parts.coercion(), AbiCoercion::interface());
    }
    assert_eq!(
        signature.argument_location(2),
        Some(AbiArgumentLocation::Parts { first: 5, count: 2 })
    );
}

#[test]
fn win64_c_scalars_and_indirect_values_use_the_native_convention() {
    for kind in [
        IntegerKind::SIGNED_8,
        IntegerKind::UNSIGNED_8,
        IntegerKind::SIGNED_16,
        IntegerKind::UNSIGNED_16,
    ] {
        assert_eq!(
            c_integer_extension(ProcedureAbi::Win64, &CType::Integer(kind)),
            CIntegerExtension::None
        );
    }
    assert_eq!(
        c_integer_extension(ProcedureAbi::Win64, &CType::Boolean),
        CIntegerExtension::Zero
    );
    let large = storage(LirType::Aggregate(vec![LirType::I64; 3]), 24);
    assert_eq!(
        c_indirect_passing(ProcedureAbi::Win64, &large),
        CIndirectPassing::CallerCopy
    );
}
