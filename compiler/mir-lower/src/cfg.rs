//! Structured MIR construction form to the output MIR CFG.

use la_arena::Arena;
use scoop_ast::Span;
use scoop_mir as mir;

use crate::structured as smir;

#[derive(Clone, Copy)]
struct UnwindTarget {
    pad: mir::BlockId,
    continuation: mir::BlockId,
    handles_in_function: bool,
}

#[derive(Clone)]
enum ReturnCleanup<'a> {
    Finally {
        owner_unwind: mir::BlockId,
        body: &'a [smir::Statement],
    },
    EndCatch {
        cleanup_pad: mir::BlockId,
    },
}

pub(crate) fn lower(body: smir::Body, return_ty: mir::Type) -> mir::Body {
    let smir::Body { locals, statements } = body;
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let mut lowerer = CfgLowerer {
        locals,
        blocks,
        entry,
        current: entry,
        current_sealed: false,
        block_count: 0,
        hidden_count: 0,
        return_ty,
        try_stack: Vec::new(),
        return_cleanups: Vec::new(),
    };
    lowerer.lower_statements(&statements);
    if !lowerer.current_sealed {
        let terminator = if lowerer.return_ty == mir::Type::Unit {
            mir::Terminator::Return { value: None }
        } else {
            mir::Terminator::Unreachable
        };
        lowerer.seal(terminator);
    }
    mir::Body {
        locals: lowerer.locals,
        blocks: lowerer.blocks,
        entry: lowerer.entry,
    }
}

struct CfgLowerer<'a> {
    locals: Arena<mir::Local>,
    blocks: Arena<mir::BasicBlock>,
    entry: mir::BlockId,
    current: mir::BlockId,
    current_sealed: bool,
    block_count: usize,
    hidden_count: usize,
    return_ty: mir::Type,
    try_stack: Vec<UnwindTarget>,
    return_cleanups: Vec<ReturnCleanup<'a>>,
}

impl<'a> CfgLowerer<'a> {
    fn active_unwind(&self) -> Option<mir::BlockId> {
        self.try_stack.last().map(|target| target.pad)
    }

    fn new_block_with(&mut self, base: &str, unwind: Option<mir::BlockId>) -> mir::BlockId {
        self.block_count += 1;
        self.blocks.alloc(mir::BasicBlock {
            name: format!("{base}.{}", self.block_count),
            statements: Vec::new(),
            terminator: mir::Terminator::Unreachable,
            unwind,
        })
    }

    fn new_block(&mut self, base: &str) -> mir::BlockId {
        self.new_block_with(base, self.active_unwind())
    }

    fn enter(&mut self, block: mir::BlockId) {
        self.current = block;
        self.current_sealed = false;
    }

    fn seal(&mut self, terminator: mir::Terminator) {
        assert!(
            !self.current_sealed,
            "a MIR block is terminated exactly once"
        );
        self.blocks[self.current].terminator = terminator;
        self.current_sealed = true;
    }

    /// A MIR block has one explicit unwind successor. Split whenever the
    /// lexical exception context changes after the block has received work.
    fn ensure_unwind_context(&mut self) {
        if self.current_sealed {
            return;
        }
        let desired = self.active_unwind();
        if self.blocks[self.current].unwind == desired {
            return;
        }
        if self.blocks[self.current].statements.is_empty() {
            self.blocks[self.current].unwind = desired;
            return;
        }
        let next = self.new_block_with("scope", desired);
        self.seal(mir::Terminator::Goto(next));
        self.enter(next);
    }

    fn push(&mut self, kind: mir::StatementKind, span: Span) {
        self.ensure_unwind_context();
        self.blocks[self.current]
            .statements
            .push(mir::Statement { kind, span });
    }

    fn lower_statements(&mut self, statements: &'a [smir::Statement]) {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.ensure_unwind_context();
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &'a smir::Statement) {
        let span = statement.span;
        match &statement.kind {
            smir::StatementKind::Expr(expr) => {
                if let Some(message) = trap_message(expr) {
                    self.seal(mir::Terminator::Trap { message });
                } else {
                    let is_call = matches!(expr, smir::Expr::Call(_));
                    let expr = self.lower_expr(expr, span);
                    if !is_call && !matches!(expr, mir::Expr::UnitLiteral) {
                        self.push(mir::StatementKind::Expr(expr), span);
                    }
                }
            }
            smir::StatementKind::ValDecl { local, init } => {
                if let smir::Expr::Call(call) = init
                    && call.return_ty != mir::Type::Unit
                {
                    self.lower_call(call, Some(*local), span);
                    return;
                }
                let init = self.lower_expr(init, span);
                self.push(
                    mir::StatementKind::ValDecl {
                        local: *local,
                        init,
                    },
                    span,
                );
            }
            smir::StatementKind::Assign { local, value } => {
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::Assign {
                        local: *local,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::GlobalAssign { global, value } => {
                let value = self.lower_expr(value, statement.span);
                self.push(
                    mir::StatementKind::GlobalAssign {
                        global: *global,
                        value,
                    },
                    statement.span,
                );
            }
            smir::StatementKind::ArraySet {
                array,
                index,
                value,
            } => {
                let array = self.lower_expr(array, span);
                let index = self.lower_expr(index, span);
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::ArraySet {
                        array,
                        index,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                let object = self.lower_expr(object, span);
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::FieldSet {
                        object,
                        index: *index,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::Return { value } => self.lower_return(value.as_ref(), span),
            smir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => self.lower_if(cond, then_body, else_body.as_deref(), span),
            smir::StatementKind::While { cond, body } => self.lower_while(cond, body, span),
            smir::StatementKind::Try(try_) => self.lower_try(try_),
            smir::StatementKind::Throw(exception) => {
                let exception = self.lower_expr(exception, span);
                let unwind = self.active_unwind();
                self.seal(mir::Terminator::Throw { exception, unwind });
            }
        }
    }

    fn lower_return(&mut self, value: Option<&smir::Expr>, span: Span) {
        let mut result = value.map(|value| self.lower_expr(value, span));
        if !self.return_cleanups.is_empty() {
            if self.return_ty == mir::Type::Unit {
                if let Some(value) = result.take() {
                    self.push(mir::StatementKind::Expr(value), span);
                }
            } else {
                let local = self.new_hidden("return", self.return_ty.clone());
                self.push(
                    mir::StatementKind::ValDecl {
                        local,
                        init: result
                            .take()
                            .expect("non-Unit returns carry a value before MIR CFG lowering"),
                    },
                    span,
                );
                result = Some(mir::Expr::Local(local));
            }
            self.emit_return_cleanups();
            if self.current_sealed {
                return;
            }
        }
        self.seal(mir::Terminator::Return { value: result });
    }

    fn lower_if(
        &mut self,
        cond: &smir::Expr,
        then_body: &'a [smir::Statement],
        else_body: Option<&'a [smir::Statement]>,
        span: Span,
    ) {
        let cond = self.lower_expr(cond, span);
        let then_block = self.new_block("if.then");
        let else_block = else_body.map(|_| self.new_block("if.else"));
        let merge_block = self.new_block("if.merge");
        self.seal(mir::Terminator::Branch {
            cond,
            then_block,
            else_block: else_block.unwrap_or(merge_block),
        });
        self.enter(then_block);
        self.lower_statements(then_body);
        if !self.current_sealed {
            self.seal(mir::Terminator::Goto(merge_block));
        }
        if let (Some(else_body), Some(else_block)) = (else_body, else_block) {
            self.enter(else_block);
            self.lower_statements(else_body);
            if !self.current_sealed {
                self.seal(mir::Terminator::Goto(merge_block));
            }
        }
        self.enter(merge_block);
    }

    fn lower_while(&mut self, cond: &smir::Expr, body: &'a [smir::Statement], span: Span) {
        let cond_block = self.new_block("while.cond");
        self.seal(mir::Terminator::Goto(cond_block));
        self.enter(cond_block);
        let cond = self.lower_expr(cond, span);
        let body_block = self.new_block("while.body");
        let exit_block = self.new_block("while.exit");
        self.seal(mir::Terminator::Branch {
            cond,
            then_block: body_block,
            else_block: exit_block,
        });
        self.enter(body_block);
        self.lower_statements(body);
        if !self.current_sealed {
            self.seal(mir::Terminator::Goto(cond_block));
        }
        self.enter(exit_block);
    }

    fn lower_try(&mut self, try_: &'a smir::Try) {
        let enclosing = self.try_stack.last().copied();
        let unwind = self.new_block_with("try.unwind", None);
        let dispatch = self.new_block_with("try.dispatch", None);
        let handler_target = if try_.catches.is_empty() {
            None
        } else {
            Some(UnwindTarget {
                pad: self.new_block_with("try.handler_pad", None),
                continuation: self
                    .new_block_with("try.handler_cleanup", enclosing.map(|target| target.pad)),
                handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
            })
        };
        let exit_target = UnwindTarget {
            pad: self.new_block_with("try.exit_pad", None),
            continuation: self
                .new_block_with("try.exit_cleanup", enclosing.map(|target| target.pad)),
            handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
        };
        let end = self.new_block_with("try.end", enclosing.map(|target| target.pad));
        let own_target = UnwindTarget {
            pad: unwind,
            continuation: dispatch,
            handles_in_function: true,
        };

        let body_entry = self.new_block_with("try.body", Some(unwind));
        self.seal(mir::Terminator::Goto(body_entry));
        self.try_stack.push(own_target);
        if let Some(finally) = &try_.finally_body {
            self.return_cleanups.push(ReturnCleanup::Finally {
                owner_unwind: unwind,
                body: finally,
            });
        }
        self.enter(body_entry);
        self.lower_statements(&try_.body);
        self.try_stack.pop();
        let finally = try_.finally_body.as_deref();
        if finally.is_some() {
            self.return_cleanups.pop();
        }

        let mut end_reachable = false;
        if !self.current_sealed {
            self.ensure_unwind_context();
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                self.seal(mir::Terminator::Goto(end));
                end_reachable = true;
            }
        }

        self.enter(unwind);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::LandingPad { cleanup: false }),
            synthetic_span(),
        );
        self.seal(mir::Terminator::Goto(dispatch));

        self.enter(dispatch);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::BeginCatch),
            synthetic_span(),
        );
        for catch in &try_.catches {
            let handler_target = handler_target.expect("a catch has a handler cleanup");
            let catch_block = self.new_block_with("try.catch", Some(handler_target.pad));
            let next = self.new_block_with("try.next", None);
            self.seal(mir::Terminator::Branch {
                cond: mir::Expr::IsInstance {
                    operand: Box::new(mir::Expr::CaughtException),
                    check_ty: catch.ty.clone(),
                },
                then_block: catch_block,
                else_block: next,
            });
            self.enter(catch_block);
            self.push(
                mir::StatementKind::ValDecl {
                    local: catch.local,
                    init: mir::Expr::Retype {
                        operand: Box::new(mir::Expr::CaughtException),
                        ty: catch.ty.clone(),
                    },
                },
                catch.span,
            );
            self.try_stack.push(handler_target);
            let cleanup_base = self.return_cleanups.len();
            if let Some(finally) = finally {
                self.return_cleanups.push(ReturnCleanup::Finally {
                    owner_unwind: unwind,
                    body: finally,
                });
            }
            self.return_cleanups.push(ReturnCleanup::EndCatch {
                cleanup_pad: handler_target.pad,
            });
            self.lower_statements(&catch.body);
            self.return_cleanups.truncate(cleanup_base);
            self.try_stack.pop();
            if !self.current_sealed {
                self.push(
                    mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                    synthetic_span(),
                );
                if let Some(finally) = finally {
                    self.lower_statements(finally);
                }
                if !self.current_sealed {
                    self.seal(mir::Terminator::Goto(end));
                    end_reachable = true;
                }
            }
            self.enter(next);
        }

        self.try_stack.push(exit_target);
        let cleanup_base = self.return_cleanups.len();
        self.return_cleanups.push(ReturnCleanup::EndCatch {
            cleanup_pad: exit_target.pad,
        });
        if let Some(finally) = finally {
            self.lower_statements(finally);
        }
        self.return_cleanups.truncate(cleanup_base);
        if !self.current_sealed {
            self.seal(mir::Terminator::Rethrow {
                unwind: Some(exit_target.pad),
            });
        }
        self.try_stack.pop();

        if let Some(handler_target) = handler_target {
            self.enter(handler_target.pad);
            self.push(
                mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: !handler_target.handles_in_function,
                }),
                synthetic_span(),
            );
            self.seal(mir::Terminator::Goto(handler_target.continuation));

            self.enter(handler_target.continuation);
            self.push(
                mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                synthetic_span(),
            );
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                match enclosing {
                    Some(target) => self.seal(mir::Terminator::Goto(target.continuation)),
                    None => self.seal(mir::Terminator::Resume),
                }
            }
        }

        self.enter(exit_target.pad);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: !exit_target.handles_in_function,
            }),
            synthetic_span(),
        );
        self.seal(mir::Terminator::Goto(exit_target.continuation));

        self.enter(exit_target.continuation);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::EndCatch),
            synthetic_span(),
        );
        match enclosing {
            Some(target) => self.seal(mir::Terminator::Goto(target.continuation)),
            None => self.seal(mir::Terminator::Resume),
        }

        self.enter(end);
        if !end_reachable {
            self.seal(mir::Terminator::Unreachable);
        }
    }

    fn emit_return_cleanups(&mut self) {
        let all = std::mem::take(&mut self.return_cleanups);
        let saved_try_stack = self.try_stack.clone();
        let mut remaining = all.clone();
        while let Some(cleanup) = remaining.pop() {
            if self.current_sealed {
                break;
            }
            self.return_cleanups = remaining.clone();
            match cleanup {
                ReturnCleanup::Finally { owner_unwind, body } => {
                    if let Some(pos) = self
                        .try_stack
                        .iter()
                        .rposition(|target| target.pad == owner_unwind)
                    {
                        self.try_stack.truncate(pos);
                    }
                    self.lower_statements(body);
                }
                ReturnCleanup::EndCatch { cleanup_pad } => {
                    self.push(
                        mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                        synthetic_span(),
                    );
                    let active = self.try_stack.pop();
                    assert_eq!(
                        active.map(|target| target.pad),
                        Some(cleanup_pad),
                        "active catch cleanup nesting"
                    );
                }
            }
        }
        self.return_cleanups = all;
        self.try_stack = saved_try_stack;
    }

    /// Normalize an expression into a call-free MIR expression, emitting
    /// calls in source evaluation order as explicit effect statements.
    fn lower_expr(&mut self, expr: &smir::Expr, span: Span) -> mir::Expr {
        match expr {
            smir::Expr::StringConst(id) => mir::Expr::StringConst(*id),
            smir::Expr::IntLiteral(value) => mir::Expr::IntLiteral(*value),
            smir::Expr::BoolLiteral(value) => mir::Expr::BoolLiteral(*value),
            smir::Expr::UnitLiteral => mir::Expr::UnitLiteral,
            smir::Expr::TupleLiteral(elements) => mir::Expr::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, span))
                    .collect(),
            ),
            smir::Expr::StructInit { struct_id, args } => mir::Expr::StructInit {
                struct_id: *struct_id,
                args: args.iter().map(|arg| self.lower_expr(arg, span)).collect(),
            },
            smir::Expr::ClassInit { class_id, args } => mir::Expr::ClassInit {
                class_id: *class_id,
                args: args.iter().map(|arg| self.lower_expr(arg, span)).collect(),
            },
            smir::Expr::ClosureAlloc { class, captures } => mir::Expr::ClosureAlloc {
                class: *class,
                captures: captures
                    .iter()
                    .map(|capture| self.lower_expr(capture, span))
                    .collect(),
            },
            smir::Expr::ClosureCapture {
                closure,
                class,
                index,
            } => mir::Expr::ClosureCapture {
                closure: Box::new(self.lower_expr(closure, span)),
                class: *class,
                index: *index,
            },
            smir::Expr::Local(local) => mir::Expr::Local(*local),
            smir::Expr::GlobalRead(global) => mir::Expr::GlobalRead(*global),
            smir::Expr::PtrFromUInt { operand, pointee } => mir::Expr::PtrFromUInt {
                operand: Box::new(self.lower_expr(operand, span)),
                pointee: pointee.clone(),
            },
            smir::Expr::PtrToUInt(operand) => {
                mir::Expr::PtrToUInt(Box::new(self.lower_expr(operand, span)))
            }
            smir::Expr::PtrCast { operand, pointee } => mir::Expr::PtrCast {
                operand: Box::new(self.lower_expr(operand, span)),
                pointee: pointee.clone(),
            },
            smir::Expr::PtrLoad {
                pointer,
                pointee,
                offset,
            } => mir::Expr::PtrLoad {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, span))),
            },
            smir::Expr::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => mir::Expr::PtrStore {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, span))),
                value: Box::new(self.lower_expr(value, span)),
            },
            smir::Expr::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => mir::Expr::PtrOffset {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: Box::new(self.lower_expr(offset, span)),
                subtract: *subtract,
            },
            smir::Expr::AddressOf { local, pointee } => mir::Expr::AddressOf {
                local: *local,
                pointee: pointee.clone(),
            },
            smir::Expr::GlobalAddress { global, pointee } => mir::Expr::GlobalAddress {
                global: *global,
                pointee: pointee.clone(),
            },
            smir::Expr::SizeOf(ty) => mir::Expr::SizeOf(ty.clone()),
            smir::Expr::AlignOf(ty) => mir::Expr::AlignOf(ty.clone()),
            smir::Expr::FunPtrNull(signature) => mir::Expr::FunPtrNull(*signature),
            smir::Expr::FunctionAddress { callback } => mir::Expr::FunctionAddress {
                callback: *callback,
            },
            smir::Expr::ForeignCallbackRegister { bridge, closure } => {
                mir::Expr::ForeignCallbackRegister {
                    bridge: *bridge,
                    closure: Box::new(self.lower_expr(closure, span)),
                }
            }
            smir::Expr::ForeignCallbackOperation {
                operation,
                callback,
                result_ty,
            } => mir::Expr::ForeignCallbackOperation {
                operation: *operation,
                callback: Box::new(self.lower_expr(callback, span)),
                result_ty: result_ty.clone(),
            },
            smir::Expr::Retype { operand, ty } => mir::Expr::Retype {
                operand: Box::new(self.lower_expr(operand, span)),
                ty: ty.clone(),
            },
            smir::Expr::FieldAccess { receiver, index } => mir::Expr::FieldAccess {
                receiver: Box::new(self.lower_expr(receiver, span)),
                index: *index,
            },
            smir::Expr::Call(call) => self.lower_call(call, None, span),
            smir::Expr::Box(operand) => mir::Expr::Box(Box::new(self.lower_expr(operand, span))),
            smir::Expr::Unbox(operand) => {
                mir::Expr::Unbox(Box::new(self.lower_expr(operand, span)))
            }
            smir::Expr::IsInstance { operand, check_ty } => mir::Expr::IsInstance {
                operand: Box::new(self.lower_expr(operand, span)),
                check_ty: check_ty.clone(),
            },
            smir::Expr::ArrayLiteral(elements) => mir::Expr::ArrayLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, span))
                    .collect(),
            ),
            smir::Expr::ArrayGet { array, index } => mir::Expr::ArrayGet {
                array: Box::new(self.lower_expr(array, span)),
                index: Box::new(self.lower_expr(index, span)),
            },
            smir::Expr::ArrayLen(operand) => {
                mir::Expr::ArrayLen(Box::new(self.lower_expr(operand, span)))
            }
            smir::Expr::ArrayClone(operand) => {
                mir::Expr::ArrayClone(Box::new(self.lower_expr(operand, span)))
            }
            smir::Expr::ShortCircuit {
                op: smir::LogicOp::And,
                lhs,
                rhs,
            } => self.lower_short_circuit(lhs, rhs, false, span),
            smir::Expr::ShortCircuit {
                op: smir::LogicOp::Or,
                lhs,
                rhs,
            } => self.lower_short_circuit(lhs, rhs, true, span),
            smir::Expr::Binary { op, lhs, rhs } => {
                let lhs = self.lower_expr(lhs, span);
                let rhs = self.lower_expr(rhs, span);
                mir::Expr::Binary {
                    op: *op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::Expr::Unary { op, operand } => mir::Expr::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand, span)),
            },
            smir::Expr::VariantConstruct {
                ty,
                variant,
                fields,
            } => mir::Expr::VariantConstruct {
                ty: ty.clone(),
                variant: *variant,
                fields: fields
                    .iter()
                    .map(|field| self.lower_expr(field, span))
                    .collect(),
            },
            smir::Expr::EnumTag(operand) => {
                mir::Expr::EnumTag(Box::new(self.lower_expr(operand, span)))
            }
            smir::Expr::EnumField {
                operand,
                variant,
                index,
            } => mir::Expr::EnumField {
                operand: Box::new(self.lower_expr(operand, span)),
                variant: *variant,
                index: *index,
            },
        }
    }

    fn lower_short_circuit(
        &mut self,
        lhs: &smir::Expr,
        rhs: &smir::Expr,
        short_value: bool,
        span: Span,
    ) -> mir::Expr {
        let lhs = self.lower_expr(lhs, span);
        let rhs_block = self.new_block("logic.rhs");
        let short_block = self.new_block("logic.short");
        let merge = self.new_block("logic.merge");
        let (then_block, else_block) = if short_value {
            (short_block, rhs_block)
        } else {
            (rhs_block, short_block)
        };
        self.seal(mir::Terminator::Branch {
            cond: lhs,
            then_block,
            else_block,
        });

        let result = self.new_hidden("logic", mir::Type::Boolean);
        self.enter(short_block);
        self.push(
            mir::StatementKind::Assign {
                local: result,
                value: mir::Expr::BoolLiteral(short_value),
            },
            span,
        );
        self.seal(mir::Terminator::Goto(merge));

        self.enter(rhs_block);
        let rhs = self.lower_expr(rhs, span);
        self.push(
            mir::StatementKind::Assign {
                local: result,
                value: rhs,
            },
            span,
        );
        self.seal(mir::Terminator::Goto(merge));

        self.enter(merge);
        mir::Expr::Local(result)
    }

    fn lower_call(
        &mut self,
        call: &smir::Call,
        destination: Option<mir::LocalId>,
        span: Span,
    ) -> mir::Expr {
        let args = call
            .args
            .iter()
            .map(|arg| self.lower_expr(arg, span))
            .collect();
        let normalized = mir::Call {
            target: call.target.clone(),
            args,
        };
        if call.return_ty == mir::Type::Unit {
            assert!(
                destination.is_none(),
                "Unit calls do not have MIR destinations"
            );
            self.push(
                mir::StatementKind::Call(mir::CallEffect::Unit(normalized)),
                span,
            );
            mir::Expr::UnitLiteral
        } else {
            let destination =
                destination.unwrap_or_else(|| self.new_hidden("call", call.return_ty.clone()));
            self.push(
                mir::StatementKind::Call(mir::CallEffect::Value {
                    destination,
                    call: normalized,
                }),
                span,
            );
            mir::Expr::Local(destination)
        }
    }

    fn new_hidden(&mut self, prefix: &str, ty: mir::Type) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable: false,
        })
    }
}

fn trap_message(expr: &smir::Expr) -> Option<mir::StringConstId> {
    let smir::Expr::Call(call) = expr else {
        return None;
    };
    if call.target.callee != mir::Callee::Runtime(mir::RuntimeFn::Trap) {
        return None;
    }
    let [smir::Expr::StringConst(message)] = call.args.as_slice() else {
        panic!("the trap intrinsic always carries one string constant")
    };
    Some(*message)
}

fn synthetic_span() -> Span {
    Span { start: 0, end: 0 }
}
