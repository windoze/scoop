use super::*;

impl Lowerer {
    pub(super) fn lower_initialization_ensure(
        &mut self,
        module: &hir::Module,
        unit: mir::InitializationUnitId,
        span: Span,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        // The implicit cycle message crosses the same type boundary as a
        // source expression, even when no source body mentions String.
        let message_ty = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        }
        .lower(
            module.string,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let declaration = &self.initialization_units[unit];
        let initializer = declaration.initializer;
        let cycle_thrower = declaration.cycle_thrower;
        let throwable_carrier = mir::Type::Any;
        let mut locals = Arena::new();
        let state = locals.alloc(mir::Local {
            name: "$init.state".to_string(),
            ty: mir::Type::MachineScalar(mir::MachineScalarKind::InitializationOutcome),
            mutable: false,
        });
        let caught = locals.alloc(mir::Local {
            name: "$init.caught".to_string(),
            ty: throwable_carrier.clone(),
            mutable: false,
        });
        let failure = locals.alloc(mir::Local {
            name: "$init.failure".to_string(),
            ty: throwable_carrier.clone(),
            mutable: false,
        });
        let message = locals.alloc(mir::Local {
            name: "$init.message".to_string(),
            ty: message_ty.clone(),
            mutable: false,
        });

        let enter = statement(
            smir::StatementKind::ValDecl {
                local: state,
                init: runtime_call(
                    mir::RuntimeFn::InitializationEnter,
                    vec![unit_address(unit)],
                    mir::Type::MachineScalar(mir::MachineScalarKind::InitializationOutcome),
                ),
            },
            span,
        );
        let run = vec![statement(
            smir::StatementKind::Try(smir::Try {
                body: vec![
                    statement(
                        smir::StatementKind::Expr(direct_call(
                            initializer,
                            Vec::new(),
                            mir::Type::Unit,
                        )),
                        span,
                    ),
                    statement(
                        smir::StatementKind::Expr(runtime_call(
                            mir::RuntimeFn::InitializationSucceed,
                            vec![unit_address(unit)],
                            mir::Type::Unit,
                        )),
                        span,
                    ),
                ],
                catches: vec![smir::CatchClause {
                    local: caught,
                    ty: Box::new(throwable_carrier.clone()),
                    body: vec![
                        statement(
                            smir::StatementKind::Expr(runtime_call(
                                mir::RuntimeFn::InitializationFail,
                                vec![
                                    unit_address(unit),
                                    smir::Expr::local(caught, throwable_carrier.clone()),
                                ],
                                mir::Type::Unit,
                            )),
                            span,
                        ),
                        statement(
                            smir::StatementKind::Throw(smir::Expr::local(
                                caught,
                                throwable_carrier.clone(),
                            )),
                            span,
                        ),
                    ],
                    span,
                }],
                finally_body: None,
            }),
            span,
        )];
        let failed = vec![
            statement(
                smir::StatementKind::ValDecl {
                    local: failure,
                    init: runtime_call(
                        mir::RuntimeFn::InitializationFailure,
                        vec![unit_address(unit)],
                        throwable_carrier.clone(),
                    ),
                },
                span,
            ),
            statement(
                smir::StatementKind::Throw(smir::Expr::local(failure, throwable_carrier.clone())),
                span,
            ),
        ];
        let cycle_body = vec![
            statement(
                smir::StatementKind::ValDecl {
                    local: message,
                    init: runtime_call(
                        mir::RuntimeFn::InitializationCycleMessage,
                        vec![unit_address(unit)],
                        message_ty.clone(),
                    ),
                },
                span,
            ),
            statement(
                smir::StatementKind::Expr(initialization_cycle_call(
                    cycle_thrower,
                    smir::Expr::local(message, message_ty),
                )),
                span,
            ),
            statement(smir::StatementKind::Unreachable, span),
        ];
        let terminal = vec![statement(
            smir::StatementKind::If {
                cond: initialization_outcome(state, mir::InitializationOutcome::Failed),
                then_body: failed,
                else_body: Some(cycle_body),
            },
            span,
        )];
        let ready_or_terminal = vec![statement(
            smir::StatementKind::If {
                cond: initialization_outcome(state, mir::InitializationOutcome::Ready),
                then_body: Vec::new(),
                else_body: Some(terminal),
            },
            span,
        )];
        let dispatch = statement(
            smir::StatementKind::If {
                cond: initialization_outcome(state, mir::InitializationOutcome::RunInitializer),
                then_body: run,
                else_body: Some(ready_or_terminal),
            },
            span,
        );
        (
            Vec::new(),
            mir::Type::Unit,
            smir::Body {
                locals,
                statements: vec![enter, dispatch],
                coroutine_eh: None,
            },
        )
    }
}

fn statement(kind: smir::StatementKind, span: Span) -> smir::Statement {
    smir::Statement { kind, span }
}

fn unit_address(unit: mir::InitializationUnitId) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Ptr(Box::new(mir::Type::Unit)),
        smir::ExprKind::InitializationUnitAddress(unit),
    )
}

fn runtime_call(
    function: mir::RuntimeFn,
    args: Vec<smir::Expr>,
    return_ty: mir::Type,
) -> smir::Expr {
    smir::Expr::new(
        return_ty.clone(),
        smir::ExprKind::Call(smir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::Runtime(function),
            },
            args,
            return_ty,
        }),
    )
}

fn direct_call(
    function: mir::FunctionId,
    args: Vec<smir::Expr>,
    return_ty: mir::Type,
) -> smir::Expr {
    smir::Expr::new(
        return_ty.clone(),
        smir::ExprKind::Call(smir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(function),
            },
            args,
            return_ty,
        }),
    )
}

fn initialization_cycle_call(
    target: mir::InitializationCycleThrower,
    message: smir::Expr,
) -> smir::Expr {
    let callee = match target {
        mir::InitializationCycleThrower::Local(function) => mir::Callee::User(function),
        mir::InitializationCycleThrower::External(callable) => mir::Callee::External(callable),
    };
    smir::Expr::new(
        mir::Type::Unit,
        smir::ExprKind::Call(smir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee,
            },
            args: vec![message],
            return_ty: mir::Type::Unit,
        }),
    )
}

fn initialization_outcome(state: mir::LocalId, expected: mir::InitializationOutcome) -> smir::Expr {
    smir::Expr::machine_eq(
        smir::Expr::local(
            state,
            mir::Type::MachineScalar(mir::MachineScalarKind::InitializationOutcome),
        ),
        mir::MachineScalarValue::InitializationOutcome(expected),
    )
}
