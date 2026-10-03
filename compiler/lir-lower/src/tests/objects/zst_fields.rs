use super::*;

#[test]
fn zero_sized_heap_fields_preserve_operand_calls_and_exact_read_values() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let value_type = mir::Type::Struct(empty);
    let class = builder.class(
        "Holder",
        None,
        &[("empty", value_type.clone()), ("number", INT)],
        empty_vtable(),
        vec![],
    );
    let receiver_type = mir::Type::Class(class);
    let receiver = builder.user_fn_body(
        "receiver",
        vec![],
        receiver_type.clone(),
        body_with_terminator(
            Arena::new(),
            vec![],
            mir::Terminator::Return {
                value: Some(expr(
                    receiver_type.clone(),
                    mir::ExprKind::ClassAlloc { class_id: class },
                )),
            },
        ),
    );
    let rhs = builder.user_fn_body(
        "rhs",
        vec![],
        value_type.clone(),
        body_with_terminator(
            Arena::new(),
            vec![],
            mir::Terminator::Return {
                value: Some(expr(
                    value_type.clone(),
                    mir::ExprKind::StructInit {
                        struct_id: empty,
                        args: vec![],
                    },
                )),
            },
        ),
    );
    let mut locals = Arena::new();
    let objects: [mir::LocalId; 4] = std::array::from_fn(|index| {
        locals.alloc(local(&format!("receiver{index}"), receiver_type.clone()))
    });
    let rhs_value = locals.alloc(local("rhs", value_type.clone()));
    let read = locals.alloc(local("read", value_type.clone()));
    let number = locals.alloc(local("number", INT));
    let main = builder.main(
        locals,
        vec![
            call_value(objects[0], user_call(receiver)),
            call_value(rhs_value, user_call(rhs)),
            stmt(mir::StatementKind::FieldSet {
                object: local_expr(objects[0], receiver_type.clone()),
                index: 0,
                value: local_expr(rhs_value, value_type.clone()),
            }),
            call_value(objects[1], user_call(receiver)),
            val_decl(
                read,
                expr(
                    value_type.clone(),
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(objects[1], receiver_type.clone())),
                        index: 0,
                    },
                ),
            ),
            call_value(objects[2], user_call(receiver)),
            stmt(mir::StatementKind::FieldSet {
                object: local_expr(objects[2], receiver_type.clone()),
                index: 1,
                value: int_expr(7),
            }),
            call_value(objects[3], user_call(receiver)),
            val_decl(
                number,
                expr(
                    INT,
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(objects[3], receiver_type.clone())),
                        index: 1,
                    },
                ),
            ),
        ],
    );
    let source = builder.finish(main);
    let exact = exact_type_record(&source, &value_type).id();
    let module = lower(source);
    let function = &module.functions[main.into_raw().into_u32() as usize];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, lir::Instruction::Call { .. }))
            .count(),
        5
    );
    let reads = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::MakeZstValue { value, .. } => Some(value.exact()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reads, [exact]);
    let memory = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::HeapLoad { offset, .. } => Some(("load", *offset)),
            lir::Instruction::HeapStore { offset, .. } => Some(("store", *offset)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(memory, [("store", 16), ("load", 16)]);
    insta::assert_snapshot!(lir::dump(&module));
}
