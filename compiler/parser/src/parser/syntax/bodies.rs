use std::collections::BTreeMap;

use scoop_ast::{self as ast, Span};
use scoop_syntax::{DeclarationData, DeclarationIndex};

use super::{Event, Parser};

mod callables;
mod calls;
mod operands;

#[derive(Debug)]
pub(super) enum BodyEvent {
    FunctionRoot(ast::RecoveredBodyRoot),
    Start(ast::BodyBlockId, ast::RecoveredBlockScope),
    Finish {
        span: Span,
        closed: bool,
        statements: Option<Vec<ast::RecoveredStatement>>,
    },
    Selector {
        receiver: ast::RecoveredReceiver,
        navigation: ast::Navigation,
        dot: Span,
        selector: Span,
    },
    Call(Box<ast::RecoveredCall>),
    Operand(ast::RecoveredOperand),
    If(Box<ast::RecoveredIf>),
    While(Box<ast::RecoveredWhile>),
    For(Box<ast::RecoveredFor>),
    DoWhile(Box<ast::RecoveredDoWhile>),
    When(Box<ast::RecoveredWhen>),
    Try(Box<ast::RecoveredTry>),
    CallableLiteral(Box<ast::RecoveredCallableLiteral>),
    CallableLabel {
        start: u32,
        label: ast::Ident,
    },
}

#[derive(Default)]
pub(super) struct BodyRecorder {
    blocks: Vec<(
        Option<DeclarationIndex>,
        ast::BodyBlockId,
        ast::RecoveredBlockScope,
    )>,
    roots: BTreeMap<usize, ast::RecoveredBodyRoot>,
    functions: BTreeMap<usize, BodyParts>,
}

#[derive(Default)]
struct BodyParts {
    blocks: Vec<ast::RecoveredBlock>,
    selectors: Vec<ast::RecoveredSelector>,
    calls: Vec<ast::RecoveredCall>,
    operands: Vec<ast::RecoveredOperand>,
    conditionals: Vec<ast::RecoveredIf>,
    while_loops: Vec<ast::RecoveredWhile>,
    for_loops: Vec<ast::RecoveredFor>,
    do_while_loops: Vec<ast::RecoveredDoWhile>,
    whens: Vec<ast::RecoveredWhen>,
    tries: Vec<ast::RecoveredTry>,
    callable_literals: Vec<ast::RecoveredCallableLiteral>,
}

impl BodyRecorder {
    pub(super) fn record(&mut self, event: BodyEvent, function: Option<DeclarationIndex>) {
        match event {
            BodyEvent::CallableLiteral(mut literal) => {
                if let Some(function) = function {
                    literal.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .callable_literals
                        .push(*literal);
                }
            }
            BodyEvent::CallableLabel { start, label } => {
                if let Some(parts) =
                    function.and_then(|function| self.functions.get_mut(&function.0))
                    && let Some(literal) = parts
                        .callable_literals
                        .iter_mut()
                        .rev()
                        .find(|literal| literal.span.start == start)
                {
                    literal.span.start = label.span.start;
                    literal.label = Some(label);
                }
            }
            BodyEvent::Try(mut try_) => {
                if let Some(function) = function {
                    try_.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .tries
                        .push(*try_);
                }
            }
            BodyEvent::When(mut when) => {
                if let Some(function) = function {
                    when.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .whens
                        .push(*when);
                }
            }
            BodyEvent::DoWhile(mut do_while) => {
                if let Some(function) = function {
                    do_while.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .do_while_loops
                        .push(*do_while);
                }
            }
            BodyEvent::For(mut for_loop) => {
                if let Some(function) = function {
                    for_loop.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .for_loops
                        .push(*for_loop);
                }
            }
            BodyEvent::While(mut while_loop) => {
                if let Some(function) = function {
                    while_loop.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .while_loops
                        .push(*while_loop);
                }
            }
            BodyEvent::If(mut conditional) => {
                if let Some(function) = function {
                    conditional.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .conditionals
                        .push(*conditional);
                }
            }
            BodyEvent::Operand(mut operand) => {
                if let Some(function) = function {
                    operand.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .operands
                        .push(operand);
                }
            }
            BodyEvent::Call(mut call) => {
                if let Some(function) = function {
                    call.block = self.block_for(function);
                    let parts = self.functions.entry(function.0).or_default();
                    parts.link_call_literals(&mut call);
                    parts.calls.push(*call);
                }
            }
            BodyEvent::FunctionRoot(root) => {
                if let Some(function) = function {
                    self.roots.insert(function.0, root);
                }
            }
            BodyEvent::Start(id, scope) => self.blocks.push((function, id, scope)),
            BodyEvent::Finish {
                span,
                closed,
                statements,
            } => {
                let (owner, id, scope) =
                    self.blocks.pop().expect("block grammar events are paired");
                if let (Some(owner), Some(statements)) = (owner, statements) {
                    let parent = self.block_for(owner);
                    self.functions
                        .entry(owner.0)
                        .or_default()
                        .blocks
                        .push(ast::RecoveredBlock {
                            id,
                            parent,
                            scope,
                            span,
                            closed,
                            statements,
                        });
                }
            }
            BodyEvent::Selector {
                receiver,
                navigation,
                dot,
                selector,
            } => {
                if let Some(function) = function {
                    let block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .selectors
                        .push(ast::RecoveredSelector {
                            block,
                            receiver,
                            navigation,
                            dot,
                            selector,
                        });
                }
            }
        }
    }

    fn block_for(&self, function: DeclarationIndex) -> Option<ast::BodyBlockId> {
        self.blocks
            .iter()
            .rev()
            .find(|(owner, _, _)| *owner == Some(function))
            .map(|(_, id, _)| *id)
    }

    pub(super) fn finish(self) -> Vec<DeclarationData<ast::RecoveredFunctionBody>> {
        assert!(self.blocks.is_empty(), "all body grammar events are closed");
        self.functions
            .into_iter()
            .filter_map(|(declaration, value)| {
                let root = self.roots.get(&declaration).copied()?;
                Some(DeclarationData {
                    declaration: DeclarationIndex(declaration),
                    value: ast::RecoveredFunctionBody {
                        root,
                        blocks: value.blocks,
                        selectors: value.selectors,
                        calls: value.calls,
                        operands: value.operands,
                        conditionals: value.conditionals,
                        while_loops: value.while_loops,
                        for_loops: value.for_loops,
                        do_while_loops: value.do_while_loops,
                        whens: value.whens,
                        tries: value.tries,
                        callable_literals: value.callable_literals,
                    },
                })
            })
            .collect()
    }
}

impl Parser {
    pub(crate) fn record_recovered_try(&mut self, try_: ast::RecoveredTry) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::Try(Box::new(try_))));
    }

    pub(crate) fn record_recovered_when(&mut self, when: ast::RecoveredWhen) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::When(Box::new(when))));
    }

    pub(crate) fn record_recovered_do_while(&mut self, do_while: ast::RecoveredDoWhile) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::DoWhile(Box::new(do_while))));
    }

    pub(crate) fn record_recovered_for(&mut self, for_loop: ast::RecoveredFor) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::For(Box::new(for_loop))));
    }

    pub(crate) fn record_recovered_while(&mut self, while_loop: ast::RecoveredWhile) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::While(Box::new(while_loop))));
    }

    pub(crate) fn record_recovered_if(&mut self, conditional: ast::RecoveredIf) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::If(Box::new(conditional))));
    }

    pub(crate) fn record_function_body_root(&mut self, root: ast::RecoveredBodyRoot) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::FunctionRoot(root)));
    }
    pub(crate) fn start_body_block(&mut self, offset: u32, scope: ast::RecoveredBlockScope) {
        self.syntax.events.push(Event::Body(BodyEvent::Start(
            ast::BodyBlockId(offset),
            scope,
        )));
    }

    pub(crate) fn finish_body_block(
        &mut self,
        span: Span,
        closed: bool,
        statements: Option<Vec<ast::RecoveredStatement>>,
    ) {
        self.syntax.events.push(Event::Body(BodyEvent::Finish {
            span,
            closed,
            statements,
        }));
    }

    pub(crate) fn record_recovered_selector(
        &mut self,
        receiver: ast::Expr,
        complete: bool,
        navigation: ast::Navigation,
        dot: Span,
    ) {
        let receiver = if complete {
            ast::RecoveredReceiver::Complete(Box::new(receiver))
        } else {
            ast::RecoveredReceiver::Unavailable(receiver.span())
        };
        let offset = self.peek().span.start;
        self.syntax.events.push(Event::Body(BodyEvent::Selector {
            receiver,
            navigation,
            dot,
            selector: Span::new(offset, offset),
        }));
    }
}
