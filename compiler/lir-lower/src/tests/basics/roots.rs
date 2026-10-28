use super::*;

#[test]
fn string_copy_restores_an_input_that_dies_at_the_source_operation() {
    let mut builder = Builder::new();
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", mir::Type::String));
    let result = locals.alloc(local("result", mir::Type::String));
    let function = builder.user_fn_full(
        "copy",
        vec![param("input", mir::Type::String, input)],
        mir::Type::Unit,
        locals,
        vec![val_decl(
            result,
            mir::Expr {
                ty: mir::Type::String,
                kind: mir::ExprKind::StringCopy(Box::new(mir::StringCopyOperation::Slice {
                    string: local_expr(input, mir::Type::String),
                    start: long_expr(0),
                    end: long_expr(1),
                })),
            },
        )],
    );
    let module = lower(builder.finish(function));
    let roots = module.functions[0]
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            lir::Instruction::StringCopy { live, .. } => Some(live.as_slice()),
            _ => None,
        })
        .unwrap();
    assert_eq!(roots.len(), 1);
    assert!(roots[0].restore);
    assert_eq!(roots[0].leaves.as_slice()[0].byte_offset, 0);
}

#[test]
fn dead_direct_arguments_remain_roots_without_continuation_restoration() {
    for (ty, indirect) in [
        (mir::Type::String, false),
        (mir::Type::Tuple(vec![mir::Type::String, LONG]), true),
    ] {
        for used_after in [false, true] {
            let mut builder = Builder::new();
            let mut locals = Arena::new();
            let value = locals.alloc(local("value", ty.clone()));
            let sink = builder.user_fn_full(
                "sink",
                vec![param("value", ty.clone(), value)],
                mir::Type::Unit,
                locals,
                Vec::new(),
            );
            let mut locals = Arena::new();
            let value = locals.alloc(local("value", ty.clone()));
            let call = || {
                call_stmt(mir::Call {
                    args: vec![local_expr(value, ty.clone())],
                    ..user_call(sink)
                })
            };
            let mut statements = vec![call()];
            if used_after {
                statements.push(call());
            }
            let caller = builder.user_fn_full(
                "caller",
                vec![param("value", ty.clone(), value)],
                mir::Type::Unit,
                locals,
                statements,
            );
            let module = lower(builder.finish(caller));
            let caller = &module.functions[1];
            let calls = caller
                .blocks
                .values()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction {
                    lir::Instruction::Call {
                        site: lir::CallSite::Managed(site),
                    } => Some(site),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let roots = calls[0].live.as_slice();
            // Indirect calls have their own by-value copy as well as the
            // original parameter when a later call still consumes it.
            assert_eq!(roots.len(), 1 + usize::from(indirect && used_after));
            assert!(
                roots
                    .iter()
                    .all(|root| root.restore == (used_after || indirect))
            );
            assert!(roots.iter().all(|root| root.leaves.as_slice().len() == 1));
            assert_eq!(calls.last().unwrap().live.as_slice()[0].restore, indirect);
        }
    }
}

#[test]
fn string_copy_roots_a_byte_prefix_even_when_the_array_dies_at_the_call() {
    let mut builder = Builder::new();
    let buffer = builder.mutable_array(
        "MutableArray<UInt8>",
        mir::Type::Integer(mir::IntegerKind::UNSIGNED_8),
    );
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", buffer.clone()));
    let result = locals.alloc(local("result", mir::Type::String));
    let function = builder.user_fn_full(
        "copy_prefix",
        vec![param("input", buffer.clone(), input)],
        mir::Type::Unit,
        locals,
        vec![val_decl(
            result,
            mir::Expr {
                ty: mir::Type::String,
                kind: mir::ExprKind::StringCopy(Box::new(
                    mir::StringCopyOperation::FromBytePrefix {
                        array: local_expr(input, buffer),
                        length: long_expr(7),
                    },
                )),
            },
        )],
    );
    let module = lower(builder.finish(function));
    let roots = module.functions[0]
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            lir::Instruction::StringCopy {
                source: lir::StringCopySource::BytePrefix { .. },
                live,
                ..
            } => Some(live.as_slice()),
            _ => None,
        })
        .unwrap();
    assert_eq!(roots.len(), 1);
    assert!(roots[0].restore);
    assert_eq!(roots[0].leaves.as_slice()[0].byte_offset, 0);
}
