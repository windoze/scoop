use super::*;

mod evaluation;
mod fixture;

use fixture::*;

#[test]
fn unit_and_empty_pointer_accesses_have_only_exact_logical_results() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let values = [
        expr(mir::Type::Unit, mir::ExprKind::UnitLiteral),
        empty_value(empty),
    ];
    let mut expressions = Vec::new();
    for value in &values {
        for offset in [None, Some(long_expr(-17))] {
            expressions.push(load(pointer(&value.ty, 4096), offset.clone()));
            expressions.push(store(pointer(&value.ty, 4096), offset, value.clone()));
        }
    }
    let source = source(builder, expressions);
    let exact: Vec<_> = values
        .iter()
        .flat_map(|value| [exact_type_record(&source, &value.ty).id(); 2])
        .collect();
    assert_ne!(exact[0], exact[2]);
    let lowered = lower(source);
    let function = &lowered.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    let mut results = Vec::new();
    for instruction in instructions {
        match instruction {
            lir::Instruction::MakeZstValue { out, value } => {
                assert_eq!(
                    &function.temps[*out].ty,
                    value.representation().storage_type()
                );
                assert_eq!(value.representation().layout().alignment().get(), 1);
                assert_eq!(value.representation().scan(), &lir::RefScan::None);
                results.push(value.exact());
            }
            lir::Instruction::RawLoad { .. }
            | lir::Instruction::RawStore { .. }
            | lir::Instruction::PtrOffset { .. } => panic!("ZST must have no payload access"),
            _ => continue,
        }
    }
    assert_eq!(results, exact);
    insta::assert_snapshot!(body_dump(&lowered));
}

#[test]
fn zst_pointer_offsets_preserve_bits_at_signed_and_compiler_offset_extremes() {
    let signed = [i64::MIN, -1, 0, 1, i64::MAX]
        .into_iter()
        .flat_map(|value| [false, true].map(move |subtract| (long_expr(value), subtract)));
    let machine = [(
        mir::Expr::machine_scalar(mir::MachineScalarValue::PointerElementOffset(u64::MAX)),
        false,
    )];
    for (delta, subtract) in signed.chain(machine) {
        let mut builder = Builder::new();
        let value = offset(pointer(&mir::Type::Unit, 4096), delta, subtract);
        let function = builder.user_fn_body(
            "displaced",
            Vec::new(),
            value.ty.clone(),
            returning_body(Arena::new(), value),
        );
        builder.functions[function].gc_effect = mir::GcEffect::NoGc;
        let main = builder.main(Arena::new(), Vec::new());
        let lowered = lower(builder.finish(main));
        let function = &lowered.functions[0];
        let block = &function.blocks[function.entry];
        let [lir::Instruction::ULongToPtr { out, value }] = block.instructions.as_slice() else {
            panic!("zero stride must emit only the original pointer conversion");
        };
        assert_eq!(
            *value,
            lir::Value::IntegerConst(lir::LirIntegerConstant::Unsigned64(4096))
        );
        assert!(
            matches!(block.terminator, lir::Terminator::Return { value: Some(lir::Value::Temp(result)) } if result == *out)
        );
    }
}

#[test]
fn nonzero_raw_storage_keeps_its_complete_layout_and_stride() {
    let lowered = lower(source(
        Builder::new(),
        vec![
            load(pointer(&INT, 4096), Some(long_expr(2))),
            store(pointer(&LONG, 8192), None, long_expr(7)),
        ],
    ));
    let function = &lowered.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    assert!(instructions.iter().any(|instruction| matches!(instruction,
        lir::Instruction::PtrOffset { element_size, .. } if element_size.get() == 4
    )));
    let values: Vec<_> = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::RawLoad { pointee, .. }
            | lir::Instruction::RawStore { pointee, .. } => Some((
                pointee.storage_type().clone(),
                pointee.layout().size().get(),
                pointee.layout().alignment().get(),
                pointee.scan().clone(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        values,
        vec![
            (lir::LirType::I32, 4, 4, lir::RefScan::None),
            (lir::LirType::I64, 8, 8, lir::RefScan::None),
        ]
    );
}

#[test]
#[should_panic(expected = "pointer offset requires Long or an additive PointerElementOffset")]
fn erased_zst_offsets_still_reject_the_wrong_integer_domain() {
    lower(source(
        Builder::new(),
        vec![load(pointer(&mir::Type::Unit, 4096), Some(ulong_expr(1)))],
    ));
}

#[test]
#[should_panic(expected = "pointer offset requires Long or an additive PointerElementOffset")]
fn erased_zst_offsets_still_reject_subtraction_of_compiler_offsets() {
    lower(source(
        Builder::new(),
        vec![offset(
            pointer(&mir::Type::Unit, 4096),
            mir::Expr::machine_scalar(mir::MachineScalarValue::PointerElementOffset(1)),
            true,
        )],
    ));
}
