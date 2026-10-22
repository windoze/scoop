use std::collections::BTreeMap;

use scoop_ast::{self as ast, Span};
use scoop_syntax::{DeclarationData, DeclarationIndex};

use super::{Event, Parser};

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
    TrailingArgument {
        call: Span,
        argument: Box<ast::RecoveredCallArgument>,
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
}

impl BodyRecorder {
    pub(super) fn record(&mut self, event: BodyEvent, function: Option<DeclarationIndex>) {
        match event {
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
            BodyEvent::TrailingArgument { call, argument } => {
                if let Some(parts) =
                    function.and_then(|function| self.functions.get_mut(&function.0))
                    && let Some(target) = parts
                        .calls
                        .iter_mut()
                        .rev()
                        .find(|target| target.span == call)
                {
                    target.span.end = argument.span.end;
                    target.trailing = Some(*argument);
                }
            }
            BodyEvent::Call(mut call) => {
                if let Some(function) = function {
                    call.block = self.block_for(function);
                    self.functions
                        .entry(function.0)
                        .or_default()
                        .calls
                        .push(*call);
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
                    },
                })
            })
            .collect()
    }
}

impl Parser {
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
