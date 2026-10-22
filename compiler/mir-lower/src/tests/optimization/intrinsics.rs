use super::*;
use crate::tests::function_adapters::function_type;

fn reference_source(
    kind: hir::IntegerIntrinsicKind,
    through_helper: bool,
) -> hir::LocalConcreteHirOutput {
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let int = h.int;
    let (signature, callable) = function_type(&mut h, vec![int], int);
    let mut helper_locals = Arena::new();
    let action = helper_locals.alloc(local("action", callable));
    let value = helper_locals.alloc(local("value", int));
    let helper = h.user_fn_full(
        "invoke",
        Vec::new(),
        vec![
            param("action", callable, action),
            param("value", int, value),
        ],
        int,
        hir::Body {
            locals: helper_locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: expr(
                    hir::ExprKind::CallableCall {
                        callee: Box::new(local_ref(action, callable)),
                        function_type: signature,
                        args: vec![local_ref(value, int)],
                    },
                    int,
                ),
            })],
        },
    );
    let mut parameters = Arena::new();
    let lhs = parameters.alloc(local("lhs", int));
    let rhs = parameters.alloc(local("rhs", int));
    let target = h.user_fn_full(
        "primitive",
        Vec::new(),
        vec![param("lhs", int, lhs), param("rhs", int, rhs)],
        int,
        hir::Body {
            locals: parameters,
            statements: Vec::new(),
        },
    );
    h.functions[target].kind = hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
        kind: hir::IntrinsicFunctionKind::Integer(kind),
        provider: hir::IntrinsicProviderId::from_raw(0),
    });
    let mut locals = Arena::new();
    let lhs = locals.alloc(local("lhs", int));
    let rhs = locals.alloc(local("rhs", int));
    let unknown = locals.alloc(local("unknown", callable));
    let result = locals.alloc(local("result", int));
    let scope = h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("lhs", int, lhs),
            param("rhs", int, rhs),
            param("unknown", callable, unknown),
        ],
        int,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let output = h.finish(main);
    let entry = output.entry();
    let mut source = output.into_module();
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(declaration)) =
        &source.function_identities[target]
    else {
        panic!("plain intrinsic declaration")
    };
    let reference = source.callable_references.alloc(hir::CallableReference {
        definition_root: hir::CallableReferenceRoot::Source(hir::LexicalDefinitionRoot::Function(
            scope,
        )),
        definition_path: scoop_identity::StructuralDefinitionPath::from_first(
            scoop_identity::StructuralPathSegment::new(
                scoop_identity::StructuralDefinitionSiteRole::CallableConversion,
                0,
            ),
            [],
        ),
        target: hir::CallableReferenceTarget::BoundIntrinsic {
            receiver: Box::new(local_ref(lhs, int)),
            declaration: declaration.id(),
            intrinsic: hir::PrimitiveMemberIntrinsic::Integer(kind),
        },
        function_type: signature,
        owner_type_arguments: Vec::new(),
        captures: Vec::new(),
        origin: definition_origin(),
        span: SPAN,
    });
    let invoke = |callee| {
        expr(
            hir::ExprKind::CallableCall {
                callee: Box::new(callee),
                function_type: signature,
                args: vec![local_ref(rhs, int)],
            },
            int,
        )
    };
    let hir::FunctionKind::User(body) = &mut source.functions[scope].kind else {
        unreachable!()
    };
    let reference = expr(hir::ExprKind::CallableReference(reference), callable);
    body.statements = vec![
        val_decl(
            result,
            if through_helper {
                call_typed(helper, vec![reference, local_ref(rhs, int)], int)
            } else {
                invoke(reference)
            },
        ),
        expr_stmt(invoke(local_ref(unknown, callable))),
        stmt(hir::StatementKind::Return {
            value: local_ref(result, int),
        }),
    ];
    scoop_hir_lower::concretize_output(&executable_output(source, entry)).unwrap()
}

#[test]
fn intrinsic_references_expand_with_ordinary_optimizations_disabled() {
    for operation in [
        hir::IntegerIntrinsicKind::NoGcOperation {
            kind: hir::IntegerKind::SIGNED_32,
            operation: hir::NoGcIntegerOperation::Add,
        },
        hir::IntegerIntrinsicKind::ManagedOperation {
            kind: hir::IntegerKind::SIGNED_32,
            operation: hir::IntegerDivRem::Div,
        },
    ] {
        let source = reference_source(operation, false);
        for options in [
            MirOptimizationOptions::default(),
            MirOptimizationOptions {
                devirtualize: true,
                ..Default::default()
            },
            MirOptimizationOptions::RELEASE,
        ] {
            let mut module = crate::lower(&source).unwrap();
            let selected = mir::SelectedExternalMirSet::empty(module.cone);
            crate::optimization::run_current(
                &mut module,
                &selected,
                source.module(),
                options,
                Default::default(),
            );
            let function = module
                .functions
                .iter()
                .find(|(_, function)| function.name == "scope")
                .unwrap()
                .1;
            let mut operations = 0;
            let mut closures = 0;
            let mut throws = 0;
            for (_, block) in function.body.blocks.iter() {
                mir::visit_block_exprs(block, &mut |expression| {
                    operations += usize::from(matches!(
                        expression.kind,
                        mir::ExprKind::IntegerBinary { .. }
                            | mir::ExprKind::SafeIntegerDivRem { .. }
                    ));
                });
                for statement in &block.statements {
                    if matches!(statement.kind, mir::StatementKind::Call(_)) {
                        closures += usize::from(matches!(
                            statement_call(statement).0.target,
                            mir::CallTarget::Closure { .. }
                        ));
                    }
                }
                throws += usize::from(matches!(block.terminator, mir::Terminator::Throw { .. }));
            }
            assert_eq!(operations, 1, "{operation:?} {options:?}");
            assert_eq!(closures, 1, "the unknown callable keeps dynamic dispatch");
            assert_eq!(
                throws,
                usize::from(matches!(
                    operation,
                    hir::IntegerIntrinsicKind::ManagedOperation { .. }
                ))
            );
            assert_mir_foundation_projection(&module);
        }
    }
}

#[test]
fn ordinary_inlining_exposes_an_intrinsic_without_optional_devirtualization() {
    let source = reference_source(
        hir::IntegerIntrinsicKind::NoGcOperation {
            kind: hir::IntegerKind::SIGNED_32,
            operation: hir::NoGcIntegerOperation::Add,
        },
        true,
    );
    let mut module = crate::lower(&source).unwrap();
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run_current(
        &mut module,
        &selected,
        source.module(),
        MirOptimizationOptions {
            inline: true,
            ..Default::default()
        },
        Default::default(),
    );
    let function = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .1;
    let mut operations = 0;
    let mut calls = 0;
    for (_, block) in function.body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expression| {
            operations += usize::from(matches!(
                expression.kind,
                mir::ExprKind::IntegerBinary { .. }
            ));
        });
        calls += block
            .statements
            .iter()
            .filter(|statement| matches!(statement.kind, mir::StatementKind::Call(_)))
            .count();
    }
    assert_eq!(operations, 1);
    assert_eq!(calls, 1, "only the unknown closure remains");
    assert_mir_foundation_projection(&module);
}
