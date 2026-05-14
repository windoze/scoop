//! 语法糖与特殊 case 的 lowering（TODO T0103e）。
//!
//! 说明：
//! - 该模块集中 delegated properties（spec §10.4）与其衍生形态（lazy/observable/vetoable）的 lowering；
//! - 规则与 span 选择尽量保持既有行为稳定，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;
use crate::ty::TypeId;

use super::HirLowering;
use super::types::*;

use super::super::{
    Block, CallArg, Expr, ExprKind, LiteralKind, MemberAccess, MemberRef, Stmt, StmtKind, ValDecl,
    ValueRef, WhenArm, WhenPat,
};

impl<'a> HirLowering<'a> {
    pub(super) fn try_lower_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<Expr> {
        let ast::ExprKind::MemberAccess { receiver, member } = &lhs.kind else {
            return None;
        };
        let resolved = self.resolved_member_for_lowering(member);
        let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref() else {
            return None;
        };
        let info = self.delegated_properties.get(fqn.as_str()).cloned()?;

        match info {
            DelegatedPropertyInfo::Observable(info) => self
                .lower_observable_delegated_property_assign(
                    pkg_prefix,
                    span,
                    member.span,
                    receiver,
                    rhs,
                    &info,
                ),
            DelegatedPropertyInfo::Vetoable(info) => self.lower_vetoable_delegated_property_assign(
                pkg_prefix,
                span,
                member.span,
                receiver,
                rhs,
                &info,
            ),
            DelegatedPropertyInfo::Generic(info) => {
                let receiver = self.lower_expr(pkg_prefix, receiver);
                let this_ref = receiver.clone();
                let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                    member.span,
                    receiver.clone(),
                    &info,
                );
                let setter_member_resolved = info.delegate_class_fqn.as_ref().map(|class_fqn| {
                    let setter_fqn = format!("{class_fqn}.setValue");
                    MemberRef::Fun {
                        id: self.symbols.intern_top_level(setter_fqn.clone()),
                        fqn: setter_fqn,
                    }
                });
                let callee = Expr {
                    span: member.span,
                    ty: self.builtins.any,
                    kind: ExprKind::MemberAccess {
                        receiver: Box::new(delegate),
                        member: MemberAccess {
                            span: member.span,
                            name: "setValue".to_string(),
                            resolved: setter_member_resolved,
                        },
                    },
                };

                let meta = self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);
                let value = self.lower_expr(pkg_prefix, rhs);

                Some(Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![
                            CallArg::Positional(this_ref),
                            CallArg::Positional(meta),
                            CallArg::Positional(value),
                        ],
                    },
                })
            }

            DelegatedPropertyInfo::Lazy(_) | DelegatedPropertyInfo::MapBacked => None,
        }
    }

    fn delegated_property_ty(&mut self, ty: Option<&ast::TypeRef>) -> TypeId {
        ty.map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any)
    }

    fn lower_delegated_property_ty(
        &mut self,
        decl: DelegatedPropertyDeclContext<'a>,
        ty: Option<&ast::TypeRef>,
    ) -> TypeId {
        self.with_foreign_ast_context(decl.source, decl.file, |this| {
            this.delegated_property_ty(ty)
        })
    }

    fn lower_delegated_property_expr(
        &mut self,
        pkg_prefix: &str,
        decl: DelegatedPropertyDeclContext<'a>,
        expr: &ast::Expr,
    ) -> Expr {
        self.with_foreign_ast_context(decl.source, decl.file, |this| {
            this.lower_expr(pkg_prefix, expr)
        })
    }

    fn member_access_to_class_field(
        &mut self,
        span: Span,
        receiver: Expr,
        member_name: String,
        field_fqn: String,
    ) -> Expr {
        let access_span = self.fresh_synthetic_call_site_span(span);
        Expr {
            span: access_span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span: access_span,
                    name: member_name,
                    resolved: Some(MemberRef::Value {
                        id: self.symbols.intern_top_level(field_fqn.clone()),
                        fqn: field_fqn,
                    }),
                },
            },
        }
    }

    pub(super) fn lower_lazy_delegated_property_get_from_receiver(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: Expr,
        info: &LazyDelegatedPropertyInfo<'a>,
    ) -> (ExprKind, TypeId) {
        let decl = info.decl;
        let inited_name = format!("{}$lazy_inited", info.name);
        let value_name = format!("{}$lazy_value", info.name);
        let value_ty = self.lower_delegated_property_ty(decl, info.ty.as_ref());

        // `LazyThreadSafetyMode.None`：沿用早期阶段的“无锁 + bool 标记”实现；
        // 其它模式：通过 `scoop.sync.Mutex` 保障并发可见性（lock/unlock 作为 acquire/release）。
        if info.mode == StdLazyThreadSafetyMode::None {
            let subject = self.member_access_to_class_field(
                span,
                receiver.clone(),
                inited_name,
                info.inited_field_fqn.clone(),
            );

            let true_body = self.member_access_to_class_field(
                span,
                receiver.clone(),
                value_name.clone(),
                info.value_field_fqn.clone(),
            );

            let init_value =
                self.lower_delegated_property_expr(pkg_prefix, decl, &info.initializer_body);
            let assign_value_span = self.fresh_synthetic_call_site_span(span);
            let assign_value = Stmt {
                span: assign_value_span,
                ty: self.builtins.unit,
                kind: StmtKind::Assign {
                    lhs: self.member_access_to_class_field(
                        span,
                        receiver.clone(),
                        value_name.clone(),
                        info.value_field_fqn.clone(),
                    ),
                    eq_span: assign_value_span,
                    rhs: init_value,
                },
            };

            let assign_inited_span = self.fresh_synthetic_call_site_span(span);
            let assign_inited = Stmt {
                span: assign_inited_span,
                ty: self.builtins.unit,
                kind: StmtKind::Assign {
                    lhs: self.member_access_to_class_field(
                        span,
                        receiver.clone(),
                        format!("{}$lazy_inited", info.name),
                        info.inited_field_fqn.clone(),
                    ),
                    eq_span: assign_inited_span,
                    rhs: Expr {
                        span,
                        ty: self.builtins.bool_,
                        kind: ExprKind::Literal(LiteralKind::Bool(true)),
                    },
                },
            };

            let tail = Stmt {
                span,
                ty: value_ty,
                kind: StmtKind::Expr(self.member_access_to_class_field(
                    span,
                    receiver,
                    value_name,
                    info.value_field_fqn.clone(),
                )),
            };

            let false_body = Expr {
                span,
                ty: value_ty,
                kind: ExprKind::Block(Block {
                    span,
                    ty: value_ty,
                    stmts: vec![assign_value, assign_inited, tail],
                }),
            };

            return (
                ExprKind::When {
                    subject: Box::new(subject),
                    arms: vec![
                        WhenArm {
                            span,
                            pat: WhenPat::BoolLit { span, value: true },
                            guard: None,
                            arrow_span: span,
                            body: true_body,
                        },
                        WhenArm {
                            span,
                            pat: WhenPat::BoolLit { span, value: false },
                            guard: None,
                            arrow_span: span,
                            body: false_body,
                        },
                    ],
                },
                value_ty,
            );
        }

        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$lazy_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$lazy_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let inited_expr = self.member_access_to_class_field(
            span,
            receiver.clone(),
            inited_name,
            info.inited_field_fqn.clone(),
        );

        let value_expr = self.member_access_to_class_field(
            span,
            receiver.clone(),
            value_name.clone(),
            info.value_field_fqn.clone(),
        );
        // 说明：早期 LLVM codegen 的 `when` 结果合流（phi）对“arm body 内部再产生分支”的情况
        // 仍较脆弱（会触发 dominance/CFG 校验失败）。为保证 run-pass 可回归，
        // 这里把 lazy 的控制流拆成：
        // 1) `lock(mutex)`
        // 2) `when (inited) { true -> Unit; false -> <init path> }`（Unit）
        // 3) 在锁内读取 value → `out`
        // 4) `unlock(mutex)` 并返回 `out`
        let out_decl_span = Span::new(span.end, span.end);
        let out_id = self.intern_local_symbol(out_decl_span, false);
        let out_name = "$lazy_out".to_string();
        let out_decl = ValDecl {
            span: out_decl_span,
            id: Some(out_id),
            name: Some(out_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(value_expr.clone()),
        };
        let out_ref_expr = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: out_id,
                name: out_name,
                decl_span: out_decl_span,
            }),
        };

        let false_arm_unit = match info.mode {
            StdLazyThreadSafetyMode::Synchronized => {
                // Synchronized：在锁内完成 initializer + 发布（只执行一次）。
                let computed_span = Span::new(span.start, span.start);
                let computed_id = self.intern_local_symbol(computed_span, false);
                let computed_name = "$lazy_computed".to_string();
                let computed_decl = ValDecl {
                    span: computed_span,
                    id: Some(computed_id),
                    name: Some(computed_name.clone()),
                    mutable: false,
                    ty: value_ty,
                    init: Some(self.lower_delegated_property_expr(
                        pkg_prefix,
                        decl,
                        &info.initializer_body,
                    )),
                };
                let computed_ref = Expr {
                    span: computed_span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: computed_id,
                        name: computed_name,
                        decl_span: computed_span,
                    }),
                };

                let assign_value_span = self.fresh_synthetic_call_site_span(span);
                let assign_value = Stmt {
                    span: assign_value_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: value_expr.clone(),
                        eq_span: assign_value_span,
                        rhs: computed_ref,
                    },
                };
                let assign_inited_span = self.fresh_synthetic_call_site_span(span);
                let assign_inited = Stmt {
                    span: assign_inited_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: inited_expr.clone(),
                        eq_span: assign_inited_span,
                        rhs: Expr {
                            span,
                            ty: self.builtins.bool_,
                            kind: ExprKind::Literal(LiteralKind::Bool(true)),
                        },
                    },
                };

                Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Block(Block {
                        span,
                        ty: self.builtins.unit,
                        stmts: vec![
                            Stmt {
                                span: computed_decl.span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Val(computed_decl),
                            },
                            assign_value,
                            assign_inited,
                        ],
                    }),
                }
            }
            StdLazyThreadSafetyMode::Publication => {
                // Publication：释放锁后执行 initializer，再二次加锁“发布”（允许 initializer 并发执行多次）。
                let computed_span = Span::new(span.start, span.start);
                let computed_id = self.intern_local_symbol(computed_span, false);
                let computed_name = "$lazy_computed".to_string();
                let computed_decl = ValDecl {
                    span: computed_span,
                    id: Some(computed_id),
                    name: Some(computed_name.clone()),
                    mutable: false,
                    ty: value_ty,
                    init: Some(self.lower_delegated_property_expr(
                        pkg_prefix,
                        decl,
                        &info.initializer_body,
                    )),
                };
                let computed_ref = Expr {
                    span: computed_span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: computed_id,
                        name: computed_name,
                        decl_span: computed_span,
                    }),
                };

                let assign_value_span = self.fresh_synthetic_call_site_span(span);
                let assign_value = Stmt {
                    span: assign_value_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: value_expr.clone(),
                        eq_span: assign_value_span,
                        rhs: computed_ref,
                    },
                };
                let assign_inited_span = self.fresh_synthetic_call_site_span(span);
                let assign_inited = Stmt {
                    span: assign_inited_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: inited_expr.clone(),
                        eq_span: assign_inited_span,
                        rhs: Expr {
                            span,
                            ty: self.builtins.bool_,
                            kind: ExprKind::Literal(LiteralKind::Bool(true)),
                        },
                    },
                };

                let publish_when = Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::When {
                        subject: Box::new(inited_expr.clone()),
                        arms: vec![
                            WhenArm {
                                span,
                                pat: WhenPat::BoolLit { span, value: true },
                                guard: None,
                                arrow_span: span,
                                body: Expr {
                                    span,
                                    ty: self.builtins.unit,
                                    kind: ExprKind::Literal(LiteralKind::Unit),
                                },
                            },
                            WhenArm {
                                span,
                                pat: WhenPat::BoolLit { span, value: false },
                                guard: None,
                                arrow_span: span,
                                body: Expr {
                                    span,
                                    ty: self.builtins.unit,
                                    kind: ExprKind::Block(Block {
                                        span,
                                        ty: self.builtins.unit,
                                        stmts: vec![assign_value, assign_inited],
                                    }),
                                },
                            },
                        ],
                    },
                };

                Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Block(Block {
                        span,
                        ty: self.builtins.unit,
                        stmts: vec![
                            unlock_stmt.clone(),
                            Stmt {
                                span: computed_decl.span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Val(computed_decl),
                            },
                            lock_stmt.clone(),
                            Stmt {
                                span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Expr(publish_when),
                            },
                        ],
                    }),
                }
            }
            StdLazyThreadSafetyMode::None => unreachable!("handled above"),
        };

        let outer_when_unit = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::When {
                subject: Box::new(inited_expr.clone()),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: true },
                        guard: None,
                        arrow_span: span,
                        body: Expr {
                            span,
                            ty: self.builtins.unit,
                            kind: ExprKind::Literal(LiteralKind::Unit),
                        },
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: false },
                        guard: None,
                        arrow_span: span,
                        body: false_arm_unit,
                    },
                ],
            },
        };

        (
            ExprKind::Block(Block {
                span,
                ty: value_ty,
                stmts: vec![
                    lock_stmt,
                    Stmt {
                        span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Expr(outer_when_unit),
                    },
                    Stmt {
                        span: out_decl.span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Val(out_decl),
                    },
                    unlock_stmt,
                    Stmt {
                        span,
                        ty: value_ty,
                        kind: StmtKind::Expr(out_ref_expr),
                    },
                ],
            }),
            value_ty,
        )
    }

    pub(super) fn lower_observable_vetoable_delegated_property_get_from_receiver(
        &mut self,
        span: Span,
        receiver: Expr,
        property_fqn: &str,
        decl: DelegatedPropertyDeclContext<'a>,
        ty: Option<&ast::TypeRef>,
        mutex_field_fqn: Option<String>,
    ) -> (ExprKind, TypeId) {
        // observable/vetoable（T1326b）：
        // - 读取需要具备并发可见性（避免 data race）；
        // - 早期阶段通过一个 per-property 的 `Mutex` 保护 backing field 读写。
        let value_ty = self.lower_delegated_property_ty(decl, ty);
        let property_name = property_fqn
            .rsplit('.')
            .next()
            .unwrap_or(property_fqn)
            .to_string();

        let mutex_fqn = mutex_field_fqn.unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__.{property_fqn}$delegate_mutex")
        });
        let mutex_name = format!("{property_name}$delegate_mutex");
        let mutex_field =
            self.member_access_to_class_field(span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let out_decl_span = Span::new(span.end, span.end);
        let out_id = self.intern_local_symbol(out_decl_span, false);
        let out_name = "$delegate_out".to_string();
        let out_decl = ValDecl {
            span: out_decl_span,
            id: Some(out_id),
            name: Some(out_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.member_access_to_class_field(
                span,
                receiver,
                property_name,
                property_fqn.to_string(),
            )),
        };
        let out_ref_expr = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: out_id,
                name: out_name,
                decl_span: out_decl_span,
            }),
        };

        (
            ExprKind::Block(Block {
                span,
                ty: value_ty,
                stmts: vec![
                    lock_stmt,
                    Stmt {
                        span: out_decl.span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Val(out_decl),
                    },
                    unlock_stmt,
                    Stmt {
                        span,
                        ty: value_ty,
                        kind: StmtKind::Expr(out_ref_expr),
                    },
                ],
            }),
            value_ty,
        )
    }

    pub(super) fn lower_observable_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        member_span: Span,
        receiver: &ast::Expr,
        rhs: &ast::Expr,
        info: &ObservableDelegatedPropertyInfo<'a>,
    ) -> Option<Expr> {
        if info.on_change.params.len() != 2 {
            return None;
        }

        let decl = info.decl;
        let value_ty = self.lower_delegated_property_ty(decl, info.ty.as_ref());
        let receiver = self.lower_expr(pkg_prefix, receiver);

        let old_param = &info.on_change.params[0];
        let new_param = &info.on_change.params[1];
        let old_name = old_param.name.text(decl.source).to_string();
        let new_name = new_param.name.text(decl.source).to_string();

        let old_id = self.intern_local_symbol(old_param.name.span, false);
        let new_id = self.intern_local_symbol(new_param.name.span, false);

        let field_access = |this: &mut Self, recv: Expr| -> Expr {
            this.member_access_to_class_field(
                member_span,
                recv,
                info.name.clone(),
                info.property_fqn.clone(),
            )
        };

        let new_decl = ValDecl {
            span: new_param.name.span,
            id: Some(new_id),
            name: Some(new_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.lower_expr(pkg_prefix, rhs)),
        };

        // 读写并发可见性：用 per-property mutex 保护 backing field。
        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$delegate_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$delegate_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(member_span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let old_decl = ValDecl {
            span: old_param.name.span,
            id: Some(old_id),
            name: Some(old_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(field_access(self, receiver.clone())),
        };

        let assign = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Assign {
                lhs: field_access(self, receiver.clone()),
                eq_span: member_span,
                rhs: Expr {
                    span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: new_id,
                        name: new_name,
                        decl_span: new_param.name.span,
                    }),
                },
            },
        };

        let callback_body =
            self.lower_delegated_property_expr(pkg_prefix, decl, &info.on_change.body);

        let block = Block {
            span,
            ty: self.builtins.unit,
            stmts: vec![
                Stmt {
                    span: new_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(new_decl),
                },
                lock_stmt,
                Stmt {
                    span: old_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(old_decl),
                },
                assign,
                unlock_stmt,
                Stmt {
                    span: callback_body.span,
                    ty: callback_body.ty,
                    kind: StmtKind::Expr(callback_body),
                },
            ],
        };

        Some(Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(block),
        })
    }

    pub(super) fn lower_vetoable_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        member_span: Span,
        receiver: &ast::Expr,
        rhs: &ast::Expr,
        info: &VetoableDelegatedPropertyInfo<'a>,
    ) -> Option<Expr> {
        if info.on_change.params.len() != 2 {
            return None;
        }

        let decl = info.decl;
        let value_ty = self.lower_delegated_property_ty(decl, info.ty.as_ref());
        let receiver = self.lower_expr(pkg_prefix, receiver);

        let old_param = &info.on_change.params[0];
        let new_param = &info.on_change.params[1];
        let old_name = old_param.name.text(decl.source).to_string();
        let new_name = new_param.name.text(decl.source).to_string();

        let old_id = self.intern_local_symbol(old_param.name.span, false);
        let new_id = self.intern_local_symbol(new_param.name.span, false);

        let field_access = |this: &mut Self, recv: Expr| -> Expr {
            this.member_access_to_class_field(
                member_span,
                recv,
                info.name.clone(),
                info.property_fqn.clone(),
            )
        };

        let new_decl = ValDecl {
            span: new_param.name.span,
            id: Some(new_id),
            name: Some(new_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.lower_expr(pkg_prefix, rhs)),
        };

        // 读写并发可见性：用 per-property mutex 保护 backing field。
        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$delegate_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$delegate_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(member_span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = |this: &mut Self, mutex: Expr| -> Stmt {
            Stmt {
                span,
                ty: this.builtins.unit,
                kind: StmtKind::Expr(this.call_top_level_fun(
                    span,
                    Self::SYNC_MUTEX_LOCK_FQN,
                    vec![mutex],
                    this.builtins.unit,
                )),
            }
        };
        let unlock_stmt = |this: &mut Self, mutex: Expr| -> Stmt {
            Stmt {
                span,
                ty: this.builtins.unit,
                kind: StmtKind::Expr(this.call_top_level_fun(
                    span,
                    Self::SYNC_MUTEX_UNLOCK_FQN,
                    vec![mutex],
                    this.builtins.unit,
                )),
            }
        };

        // 先在锁内读取 old，再解锁执行回调（避免把用户回调放在锁内导致死锁/泄漏）。
        let old_decl = ValDecl {
            span: old_param.name.span,
            id: Some(old_id),
            name: Some(old_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(field_access(self, receiver.clone())),
        };

        let ok_expr = self.lower_delegated_property_expr(pkg_prefix, decl, &info.on_change.body);

        let new_ref = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: new_id,
                name: new_name,
                decl_span: new_param.name.span,
            }),
        };

        let assign_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Assign {
                lhs: field_access(self, receiver.clone()),
                eq_span: member_span,
                rhs: new_ref,
            },
        };

        let true_block = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(Block {
                span,
                ty: self.builtins.unit,
                stmts: vec![
                    lock_stmt(self, mutex_field.clone()),
                    assign_stmt,
                    unlock_stmt(self, mutex_field.clone()),
                    Stmt {
                        span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Expr(Expr {
                            span,
                            ty: self.builtins.unit,
                            kind: ExprKind::Literal(LiteralKind::Unit),
                        }),
                    },
                ],
            }),
        };

        let false_unit = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Literal(LiteralKind::Unit),
        };

        let when_expr = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::When {
                subject: Box::new(ok_expr),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: true },
                        guard: None,
                        arrow_span: span,
                        body: true_block,
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: false },
                        guard: None,
                        arrow_span: span,
                        body: false_unit,
                    },
                ],
            },
        };

        let block = Block {
            span,
            ty: self.builtins.unit,
            stmts: vec![
                Stmt {
                    span: new_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(new_decl),
                },
                lock_stmt(self, mutex_field.clone()),
                Stmt {
                    span: old_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(old_decl),
                },
                unlock_stmt(self, mutex_field),
                Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Expr(when_expr),
                },
            ],
        };

        Some(Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(block),
        })
    }

    pub(super) fn lower_generic_delegated_property_delegate_access_expr(
        &mut self,
        span: Span,
        receiver: Expr,
        info: &GenericDelegatedPropertyInfo,
    ) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span,
                    name: format!("{}$delegate", info.name),
                    resolved: Some(MemberRef::Value {
                        id: self
                            .symbols
                            .intern_top_level(info.delegate_field_fqn.clone()),
                        fqn: info.delegate_field_fqn.clone(),
                    }),
                },
            },
        }
    }

    pub(super) fn lower_property_meta_ref_expr(&mut self, span: Span, fqn: &str) -> Expr {
        let ty = self.intern_nominal(Self::PROPERTY_META_FQN.to_string(), Vec::new(), None);
        Expr {
            span,
            ty,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.to_string()),
                fqn: fqn.to_string(),
            }),
        }
    }
}
