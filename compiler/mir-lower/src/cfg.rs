//! Structured MIR construction form to the output MIR CFG.

use la_arena::Arena;
use scoop_ast::Span;
use scoop_mir as mir;

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
        body: &'a [mir::StructuredStatement],
    },
    EndCatch {
        cleanup_pad: mir::BlockId,
    },
}

pub(crate) fn lower(body: mir::StructuredBody, return_ty: mir::Type) -> mir::Body {
    let mir::StructuredBody { locals, statements } = body;
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

    fn lower_statements(&mut self, statements: &'a [mir::StructuredStatement]) {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.ensure_unwind_context();
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &'a mir::StructuredStatement) {
        let span = statement.span;
        match &statement.kind {
            mir::StructuredStatementKind::Expr(expr) => {
                if let Some(message) = trap_message(expr) {
                    self.seal(mir::Terminator::Trap { message });
                } else {
                    self.push(mir::StatementKind::Expr(expr.clone()), span);
                }
            }
            mir::StructuredStatementKind::ValDecl { local, init } => self.push(
                mir::StatementKind::ValDecl {
                    local: *local,
                    init: init.clone(),
                },
                span,
            ),
            mir::StructuredStatementKind::Assign { local, value } => self.push(
                mir::StatementKind::Assign {
                    local: *local,
                    value: value.clone(),
                },
                span,
            ),
            mir::StructuredStatementKind::ArraySet {
                array,
                index,
                value,
            } => self.push(
                mir::StatementKind::ArraySet {
                    array: array.clone(),
                    index: index.clone(),
                    value: value.clone(),
                },
                span,
            ),
            mir::StructuredStatementKind::FieldSet {
                object,
                index,
                value,
            } => self.push(
                mir::StatementKind::FieldSet {
                    object: object.clone(),
                    index: *index,
                    value: value.clone(),
                },
                span,
            ),
            mir::StructuredStatementKind::Return { value } => {
                self.lower_return(value.as_ref(), span)
            }
            mir::StructuredStatementKind::If {
                cond,
                then_body,
                else_body,
            } => self.lower_if(cond, then_body, else_body.as_deref()),
            mir::StructuredStatementKind::While { cond, body } => self.lower_while(cond, body),
            mir::StructuredStatementKind::Try(try_) => self.lower_try(try_),
            mir::StructuredStatementKind::Throw(exception) => {
                let unwind = self.active_unwind();
                self.seal(mir::Terminator::Throw {
                    exception: exception.clone(),
                    unwind,
                });
            }
        }
    }

    fn lower_return(&mut self, value: Option<&mir::Expr>, span: Span) {
        let mut result = value.cloned();
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
        cond: &mir::Expr,
        then_body: &'a [mir::StructuredStatement],
        else_body: Option<&'a [mir::StructuredStatement]>,
    ) {
        let then_block = self.new_block("if.then");
        let else_block = else_body.map(|_| self.new_block("if.else"));
        let merge_block = self.new_block("if.merge");
        self.seal(mir::Terminator::Branch {
            cond: cond.clone(),
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

    fn lower_while(&mut self, cond: &mir::Expr, body: &'a [mir::StructuredStatement]) {
        let cond_block = self.new_block("while.cond");
        self.seal(mir::Terminator::Goto(cond_block));
        self.enter(cond_block);
        let body_block = self.new_block("while.body");
        let exit_block = self.new_block("while.exit");
        self.seal(mir::Terminator::Branch {
            cond: cond.clone(),
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

    fn lower_try(&mut self, try_: &'a mir::StructuredTry) {
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

    fn new_hidden(&mut self, prefix: &str, ty: mir::Type) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable: false,
        })
    }
}

fn trap_message(expr: &mir::Expr) -> Option<mir::StringConstId> {
    let mir::Expr::Call(call) = expr else {
        return None;
    };
    if call.target.callee != mir::Callee::Runtime(mir::RuntimeFn::Trap) {
        return None;
    }
    let [mir::Expr::StringConst(message)] = call.args.as_slice() else {
        panic!("the trap intrinsic always carries one string constant")
    };
    Some(*message)
}

fn synthetic_span() -> Span {
    Span { start: 0, end: 0 }
}
