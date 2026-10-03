use super::*;

#[test]
fn zero_sized_box_and_unbox_preserve_exact_type_without_payload_places() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let ty = mir::Type::Struct(empty);
    let boxed = builder.class(
        "box<Empty>",
        None,
        &[("value", ty.clone())],
        empty_vtable(),
        vec![],
    );
    let effect = builder.user_fn("effect", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let object = locals.alloc(local("object", mir::Type::Any));
    let value = locals.alloc(local("value", ty.clone()));
    let main = builder.main(
        locals,
        vec![
            call_stmt(user_call(effect)),
            val_decl(
                object,
                expr(
                    mir::Type::Any,
                    mir::ExprKind::Box(Box::new(expr(
                        ty.clone(),
                        mir::ExprKind::StructInit {
                            struct_id: empty,
                            args: vec![],
                        },
                    ))),
                ),
            ),
            val_decl(
                value,
                expr(
                    ty.clone(),
                    mir::ExprKind::Unbox(Box::new(local_expr(object, mir::Type::Any))),
                ),
            ),
        ],
    );
    let mut source = builder.finish(main);
    register_boxed_source_nominal(
        &mut source,
        ty.clone(),
        boxed,
        "Empty",
        SourceNominalKind::Struct,
    );
    let exact = exact_type_record(&source, &ty).id();
    let module = lower(source);
    let function = &module.functions[main.into_raw().into_u32() as usize];
    let instructions = &function.blocks[function.entry].instructions;
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, lir::Instruction::Call { .. }))
        .unwrap();
    let boxed_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, lir::Instruction::BoxValue { .. }))
        .unwrap();
    assert!(
        call < boxed_index,
        "operand effects must precede allocation"
    );
    let lir::Instruction::BoxValue {
        payload: lir::BoxPayload::ZeroSized(descriptor),
        ..
    } = &instructions[boxed_index]
    else {
        panic!("empty struct box must be the zero-sized branch")
    };
    assert_eq!(descriptor.value().exact(), exact);
    let unbox = instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::UnboxValue { result, .. } => Some(result),
            _ => None,
        })
        .unwrap();
    let lir::UnboxResult::ZeroSized {
        descriptor: expected,
        out,
    } = unbox
    else {
        panic!("ZST unbox has no destination place")
    };
    assert_eq!(descriptor, expected);
    assert_eq!(
        &function.temps[*out].ty,
        expected.value().representation().storage_type()
    );
    assert_eq!(
        function.locals.len(),
        2,
        "boxing cannot allocate an addressable empty payload"
    );
    assert!(!instructions.iter().any(|instruction| matches!(
        instruction,
        lir::Instruction::LocalAddress { .. } | lir::Instruction::HeapLoad { .. }
    )));
    let dump = lir::dump(&module);
    let operations = dump
        .lines()
        .filter(|line| line.contains("box_zst"))
        .collect::<Vec<_>>();
    insta::assert_snapshot!(operations.join("\n"));
}
