use super::*;

fn push_external_call(module: &mut Module, kind: CallKind) -> (FunctionId, BlockId) {
    let function = module
        .output
        .executable_entry()
        .expect("test module is executable");
    let block = module.functions[function].body.entry;
    let callable = ExternalCallableUseId::from_raw(0.into());
    module.functions[function].body.blocks[block]
        .statements
        .push(Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind,
                    callee: Callee::External(callable),
                },
                args: Vec::new(),
                pending: CoroutinePendingContext::Root,
            })),
            span: SourceSpan::new(0, 0).unwrap(),
        });
    (function, block)
}

#[test]
fn external_call_requires_an_imported_callable_entry() {
    let (mut module, _) = module_with_variants(Vec::new());
    let (function, block) = push_external_call(&mut module, CallKind::Direct);
    let callable = ExternalCallableUseId::from_raw(0.into());

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { function, block },
            kind: MirValidationErrorKind::InvalidExternalCallableReference { callable },
        })
    );
}
