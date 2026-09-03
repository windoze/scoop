use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

pub(super) struct ResolvedPlacePlan {
    pub(super) read: hir::Expr,
    pub(super) write: WriteCapability,
    pub(super) ty: TypeId,
}

pub(super) enum WriteCapability {
    ReadOnly,
    Direct(hir::AssignTarget),
    OperatorSet {
        receiver: hir::Expr,
        index_arguments: Vec<ast::CallArgument>,
    },
}

impl Lowerer {
    pub(crate) fn lower_update(
        &mut self,
        place: &ast::PlaceExpr,
        op: ast::UpdateOp,
        notation: ast::UpdateNotation,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let plan = self.resolve_place_plan(place, sink)?;
        if matches!(&plan.write, WriteCapability::ReadOnly) {
            self.error(span, "update operand is not writable".to_string());
            return None;
        }
        let old = self.materialize_place_expr(plan.read, "old", span, sink);
        let (kind, name) = match op {
            ast::UpdateOp::Increment => (hir::OperatorKind::Inc, "inc"),
            ast::UpdateOp::Decrement => (hir::OperatorKind::Dec, "dec"),
        };
        let name = ast::Ident {
            text: name.to_string(),
            span,
        };
        let value = self.lower_named_call_on_receiver(
            old.clone(),
            &name,
            CallSite {
                type_args: &[],
                args: &[],
                span,
            },
            sink,
            Some(plan.ty),
            RequiredCallableModifiers {
                operator: Some(kind),
                infix: false,
            },
        )?;
        if !self.is_subtype(value.ty, plan.ty) {
            self.error(
                span,
                format!(
                    "operator `{name}` returns {}, which cannot be assigned to update operand of type {}",
                    self.type_name(value.ty),
                    self.type_name(plan.ty),
                    name = name.text,
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, plan.ty);
        let new = self.materialize_place_expr(value, "new", span, sink);
        let write = self.lower_place_write(plan.write, new.clone(), span, sink)?;
        sink.push(hir::Statement { kind: write, span });
        Some(match notation {
            ast::UpdateNotation::Prefix => new,
            ast::UpdateNotation::Postfix => old,
        })
    }

    pub(super) fn materialize_place_expr(
        &mut self,
        value: hir::Expr,
        label: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = value.ty;
        let local = self.alloc_hidden(label, ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(super) fn resolve_place_plan(
        &mut self,
        place: &ast::PlaceExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedPlacePlan> {
        match place {
            ast::PlaceExpr::Name(name) => self.resolve_named_place_plan(name),
            ast::PlaceExpr::Field {
                receiver,
                name,
                span,
            } => {
                let receiver = self.lower_expr(receiver, sink, None)?;
                let receiver = self.materialize_place_expr(receiver, "place", *span, sink);
                let application = match self.types[receiver.ty] {
                    Type::Class(application) => application,
                    _ if self.is_value_ty(receiver.ty) => {
                        self.error(
                            name.span,
                            "field assignment is not supported (value types are immutable)"
                                .to_string(),
                        );
                        return None;
                    }
                    _ => {
                        let found = self.type_name(receiver.ty);
                        self.error(name.span, format!("type `{found}` has no fields"));
                        return None;
                    }
                };
                let Some((declaring, index, ty, mutable)) =
                    self.find_class_application_field(application, &name.text)
                else {
                    let class = self.classes[self.class_applications[application].template]
                        .name
                        .clone();
                    self.error(
                        name.span,
                        format!("class `{class}` has no field `{}`", name.text),
                    );
                    return None;
                };
                let field = hir::FieldRef::ClassField {
                    application: declaring,
                    index,
                };
                let read = hir::Expr {
                    kind: hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver.clone()),
                        field,
                    },
                    ty,
                    span: *span,
                    origin: self.expression_origin(*span),
                };
                let write = if mutable {
                    WriteCapability::Direct(hir::AssignTarget::Field {
                        receiver: Box::new(receiver),
                        field,
                    })
                } else {
                    WriteCapability::ReadOnly
                };
                Some(ResolvedPlacePlan { read, write, ty })
            }
            ast::PlaceExpr::Index {
                receiver,
                indices,
                span,
            } => {
                let receiver = self.lower_expr(receiver, sink, None)?;
                let receiver = self.materialize_place_expr(receiver, "place", *span, sink);
                let arguments = indices
                    .iter()
                    .cloned()
                    .map(ast::CallArgument::positional)
                    .collect::<Vec<_>>();
                let read = self.lower_named_call_on_receiver(
                    receiver.clone(),
                    &ast::Ident {
                        text: "get".to_string(),
                        span: *span,
                    },
                    CallSite {
                        type_args: &[],
                        args: &arguments,
                        span: *span,
                    },
                    sink,
                    None,
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Get),
                        infix: false,
                    },
                )?;
                let index_arguments = self.materialized_source_arguments(indices.len(), *span);
                Some(ResolvedPlacePlan {
                    ty: read.ty,
                    read,
                    write: WriteCapability::OperatorSet {
                        receiver,
                        index_arguments,
                    },
                })
            }
        }
    }

    fn resolve_named_place_plan(&mut self, name: &ast::Ident) -> Option<ResolvedPlacePlan> {
        if let Some(local) = self.scopes.lookup(&name.text) {
            let read = self.lower_var(name, None)?;
            let write = if self.locals[local].mutable {
                WriteCapability::Direct(hir::AssignTarget::Local(local))
            } else {
                WriteCapability::ReadOnly
            };
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write,
            });
        }
        if self.constructor_params_in_scope.contains_key(&name.text)
            || self.available_capture(&name.text).is_some()
        {
            let read = self.lower_var(name, None)?;
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write: WriteCapability::ReadOnly,
            });
        }
        if let Some(Type::Class(application)) =
            self.current_this_ty().map(|ty| self.types[ty].clone())
            && let Some((declaring, index, ty, mutable)) =
                self.find_class_application_field(application, &name.text)
        {
            let receiver = self.lower_current_this(name.span)?;
            let field = hir::FieldRef::ClassField {
                application: declaring,
                index,
            };
            let read = hir::Expr {
                kind: hir::ExprKind::FieldAccess {
                    receiver: Box::new(receiver.clone()),
                    field,
                },
                ty,
                span: name.span,
                origin: self.expression_origin(name.span),
            };
            let write = if mutable {
                WriteCapability::Direct(hir::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                })
            } else {
                WriteCapability::ReadOnly
            };
            return Some(ResolvedPlacePlan { read, write, ty });
        }
        if let Some(&global) = self.globals_by_name.get(&name.text) {
            if matches!(
                self.globals[global].storage,
                hir::GlobalStorage::Extern { .. }
            ) {
                self.require_unsafe_operation(name.span, "reading an extern global");
            }
            let ty = self.globals[global].ty;
            return Some(ResolvedPlacePlan {
                read: hir::Expr {
                    kind: hir::ExprKind::GlobalRead(global),
                    ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                },
                write: if self.globals[global].mutable {
                    WriteCapability::Direct(hir::AssignTarget::Global(global))
                } else {
                    WriteCapability::ReadOnly
                },
                ty,
            });
        }
        let read = self.lower_var(name, None)?;
        Some(ResolvedPlacePlan {
            ty: read.ty,
            read,
            write: WriteCapability::ReadOnly,
        })
    }

    fn materialized_source_arguments(
        &mut self,
        count: usize,
        span: Span,
    ) -> Vec<ast::CallArgument> {
        let locals = (0..count)
            .map(|index| {
                let name = format!("$argument.{index}");
                self.locals
                    .iter()
                    .filter_map(|(local, declaration)| (declaration.name == name).then_some(local))
                    .next_back()
                    .expect("a committed source call materializes every explicit input")
            })
            .collect::<Vec<_>>();
        locals
            .into_iter()
            .map(|local| self.local_source_argument(local, span))
            .collect()
    }

    fn local_source_argument(&mut self, local: hir::LocalId, span: Span) -> ast::CallArgument {
        let name = format!("$place.source.{}", self.hidden_count);
        self.hidden_count += 1;
        self.scopes.declare(name.clone(), local);
        ast::CallArgument::positional(ast::Expr::Var(ast::Ident { text: name, span }))
    }

    pub(super) fn lower_place_write(
        &mut self,
        write: WriteCapability,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        match write {
            WriteCapability::ReadOnly => {
                self.error(
                    span,
                    "compound-assignment fallback is not writable".to_string(),
                );
                None
            }
            WriteCapability::Direct(target) => Some(hir::StatementKind::Assign { target, value }),
            WriteCapability::OperatorSet {
                receiver,
                mut index_arguments,
            } => {
                let value = self.materialize_place_expr(value, "write", span, sink);
                let hir::ExprKind::Local(value_local) = value.kind else {
                    unreachable!("place values are materialized as local reads")
                };
                index_arguments.push(self.local_source_argument(value_local, span));
                let call = self.lower_named_call_on_receiver(
                    receiver,
                    &ast::Ident {
                        text: "set".to_string(),
                        span,
                    },
                    CallSite {
                        type_args: &[],
                        args: &index_arguments,
                        span,
                    },
                    sink,
                    Some(self.unit),
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Set),
                        infix: false,
                    },
                )?;
                Some(hir::StatementKind::Expr(call))
            }
        }
    }
}
