use super::*;

fn push_dependency_call(module: &mut Module, kind: CallKind) -> (FunctionId, BlockId) {
    let function = module
        .output
        .executable_entry()
        .expect("test module is executable");
    let block = module.functions[function].body.entry;
    let callable = ImportedDependencyMirCallableId::from_raw(0.into());
    module.functions[function].body.blocks[block]
        .statements
        .push(Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind,
                    callee: Callee::DependencyStrong(callable),
                },
                args: Vec::new(),
                pending: CoroutinePendingContext::Root,
            })),
            span: SourceSpan::new(0, 0).unwrap(),
        });
    (function, block)
}

#[test]
fn dependency_strong_call_requires_an_imported_callable_entry() {
    let (mut module, _) = module_with_variants(Vec::new());
    let (function, block) = push_dependency_call(&mut module, CallKind::Direct);
    let callable = ImportedDependencyMirCallableId::from_raw(0.into());

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { function, block },
            kind: MirValidationErrorKind::InvalidImportedDependencyCallableReference { callable },
        })
    );
}

#[test]
fn dependency_strong_call_cannot_claim_dynamic_dispatch() {
    let (mut module, _) = module_with_variants(Vec::new());
    let (function, block) = push_dependency_call(
        &mut module,
        CallKind::Interface {
            interface: InterfaceId::from_raw(0.into()),
            slot: 0,
        },
    );

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { function, block },
            kind: MirValidationErrorKind::ImportedDependencyCallableRequiresDirect,
        })
    );
}
