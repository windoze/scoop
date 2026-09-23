use super::*;

#[test]
fn nested_zst_store_preserves_receiver_offset_and_value_evaluation_order() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let tuple_ty = mir::Type::Tuple(vec![mir::Type::Unit, mir::Type::Struct(empty)]);
    let nested = builder.strukt("Nested", &[("value", tuple_ty.clone())]);
    let nested_ty = mir::Type::Struct(nested);
    let receiver = pointer_from(&nested_ty, load(pointer(&ULONG, 4096), None));
    let delta = load(pointer(&LONG, 8192), None);
    let write = store(pointer(&LONG, 12288), None, long_expr(7));
    let value = expr(
        nested_ty.clone(),
        mir::ExprKind::StructConstruct {
            struct_id: nested,
            fields: vec![expr(
                tuple_ty,
                mir::ExprKind::TupleLiteral(vec![write, empty_value(empty)]),
            )],
        },
    );
    let source = source(
        builder,
        vec![
            store(receiver.clone(), Some(delta.clone()), value),
            load(receiver.clone(), Some(delta.clone())),
            offset(receiver, delta, true),
        ],
    );
    let exact = exact_type_record(&source, &nested_ty).id();
    let lowered = lower(source);
    let function = &lowered.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    let operations: Vec<_> = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::RawLoad { pointee, .. } => {
                assert_eq!(pointee.storage_type(), &lir::LirType::I64);
                Some("read")
            }
            lir::Instruction::RawStore { pointee, .. } => {
                assert_eq!(pointee.storage_type(), &lir::LirType::I64);
                Some("write")
            }
            lir::Instruction::MakeZstValue { value, .. } => {
                assert_eq!(value.exact(), exact);
                Some("zst")
            }
            lir::Instruction::PtrOffset { .. } => panic!("nested ZST has no displacement"),
            _ => None,
        })
        .collect();
    assert_eq!(
        operations,
        [
            "read", "read", "write", "read", "read", "zst", "read", "read"
        ]
    );
    insta::assert_snapshot!(body_dump(&lowered));
}
