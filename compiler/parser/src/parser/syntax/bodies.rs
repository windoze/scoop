use std::collections::BTreeMap;

use scoop_ast::{self as ast, Span};
use scoop_syntax::{DeclarationData, DeclarationIndex};

use super::{Event, Parser};

#[derive(Debug)]
pub(super) enum BodyEvent {
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
}

#[derive(Default)]
pub(super) struct BodyRecorder {
    blocks: Vec<(
        Option<DeclarationIndex>,
        ast::BodyBlockId,
        ast::RecoveredBlockScope,
    )>,
    functions: BTreeMap<usize, ast::RecoveredFunctionBody>,
}

impl BodyRecorder {
    pub(super) fn record(&mut self, event: BodyEvent, function: Option<DeclarationIndex>) {
        match event {
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
            .map(|(declaration, value)| DeclarationData {
                declaration: DeclarationIndex(declaration),
                value,
            })
            .collect()
    }
}

impl Parser {
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
