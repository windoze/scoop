use super::*;

impl Lowerer {
    pub(super) fn lower_initialization_ensure(
        &mut self,
        module: &hir::Module,
        unit: mir::InitializationUnitId,
        span: Span,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let declaration = &self.initialization_units[unit];
        let initializer = declaration.initializer;
        let cycle = declaration.cycle_exception.clone();
        let throwable = mir::Type::Class(self.class_map[&module.exception_core.throwable.class()]);
        let mut locals = Arena::new();
        let state = locals.alloc(mir::Local {
            name: "$init.state".to_string(),
            ty: mir::Type::Int,
            mutable: false,
        });
        let caught = locals.alloc(mir::Local {
            name: "$init.caught".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let materialized = locals.alloc(mir::Local {
            name: "$init.materialized".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let failure = locals.alloc(mir::Local {
            name: "$init.failure".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let message = locals.alloc(mir::Local {
            name: "$init.message".to_string(),
            ty: mir::Type::String,
            mutable: false,
        });

        let enter = statement(
            smir::StatementKind::ValDecl {
                local: state,
                init: runtime_call(
                    mir::RuntimeFn::InitializationEnter,
                    vec![unit_address(unit)],
                    mir::Type::Int,
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
                    ty: Box::new(throwable.clone()),
                    body: vec![
                        statement(
                            smir::StatementKind::ValDecl {
                                local: materialized,
                                init: runtime_call(
                                    mir::RuntimeFn::MaterializeException,
                                    vec![smir::Expr::local(caught, throwable.clone())],
                                    throwable.clone(),
                                ),
                            },
                            span,
                        ),
                        statement(
                            smir::StatementKind::Expr(runtime_call(
                                mir::RuntimeFn::InitializationFail,
                                vec![
                                    unit_address(unit),
                                    smir::Expr::local(materialized, throwable.clone()),
                                ],
                                mir::Type::Unit,
                            )),
                            span,
                        ),
                        statement(
                            smir::StatementKind::Throw(smir::Expr::local(
                                materialized,
                                throwable.clone(),
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
                        throwable.clone(),
                    ),
                },
                span,
            ),
            statement(
                smir::StatementKind::Throw(smir::Expr::local(failure, throwable.clone())),
                span,
            ),
        ];
        let cycle_message = smir::Expr::local(message, mir::Type::String);
        let some_message = smir::Expr::new(
            cycle.message_type.clone(),
            smir::ExprKind::VariantConstruct {
                variant: self.option_variants.0,
                fields: vec![cycle_message],
            },
        );
        let cycle_body = vec![
            statement(
                smir::StatementKind::ValDecl {
                    local: message,
                    init: runtime_call(
                        mir::RuntimeFn::InitializationCycleMessage,
                        vec![unit_address(unit)],
                        mir::Type::String,
                    ),
                },
                span,
            ),
            statement(
                smir::StatementKind::Throw(smir::Expr::new(
                    mir::Type::Class(cycle.class),
                    smir::ExprKind::ClassNew {
                        class_id: cycle.class,
                        initializer: cycle.initializer,
                        args: vec![some_message],
                    },
                )),
                span,
            ),
        ];
        let terminal = vec![statement(
            smir::StatementKind::If {
                cond: int_state(state, 2),
                then_body: failed,
                else_body: Some(cycle_body),
            },
            span,
        )];
        let ready_or_terminal = vec![statement(
            smir::StatementKind::If {
                cond: int_state(state, 1),
                then_body: Vec::new(),
                else_body: Some(terminal),
            },
            span,
        )];
        let dispatch = statement(
            smir::StatementKind::If {
                cond: int_state(state, 0),
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

fn int_state(state: mir::LocalId, expected: i64) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Boolean,
        smir::ExprKind::Binary {
            op: mir::BinOp::IntEq,
            lhs: Box::new(smir::Expr::local(state, mir::Type::Int)),
            rhs: Box::new(smir::Expr::int(expected)),
        },
    )
}
