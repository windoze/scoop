use super::*;
use scoop_ast::{Block, RecoveredBlockScope, RecoveredStatement};

enum BlockItem {
    Statement { index: usize, complete: bool },
    Unavailable(Span),
}

impl Parser {
    pub(crate) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        self.parse_block_with_scope(RecoveredBlockScope::Block)
    }

    pub(crate) fn parse_block_with_scope(
        &mut self,
        scope: RecoveredBlockScope,
    ) -> Result<Block, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Block, |parser| {
            let open = parser.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
            parser.parse_block_after_open(open, scope)
        })
    }

    /// Lambda parsing shares the same block loop after its parameter header.
    pub(crate) fn parse_block_after_open(
        &mut self,
        open: Token,
        scope: RecoveredBlockScope,
    ) -> Result<Block, Diagnostic> {
        let checkpoint = self.checkpoint();
        self.start_body_block(open.span.start, scope);
        let body_depth = self.brace_depth();
        let mut statements = Vec::new();
        let mut items = Vec::new();
        let result = loop {
            match self.peek().kind {
                TokenKind::RBrace => {
                    let close = self.bump();
                    break Ok(Span::new(open.span.start, close.span.end));
                }
                TokenKind::Eof => break self.missing_token(TokenKind::RBrace, "`}`"),
                _ => {
                    let start = self.pos;
                    let statement_checkpoint = self.checkpoint();
                    match self.parse_statement() {
                        Ok(statement) => {
                            let end = statement.span.end;
                            items.push(BlockItem::Statement {
                                index: statements.len(),
                                complete: self.fragment_is_complete(statement_checkpoint),
                            });
                            statements.push(statement);
                            if let Err(diagnostic) = self.expect_statement_end() {
                                self.diagnostics.push(diagnostic);
                                self.synchronize_body_item(body_depth, start);
                                items.push(BlockItem::Unavailable(Span::new(
                                    end,
                                    self.body_consumed_end().max(end),
                                )));
                            }
                        }
                        Err(diagnostic) => {
                            if self.at_eof() {
                                items.push(BlockItem::Unavailable(Span::new(
                                    self.tokens[start].span.start,
                                    self.body_consumed_end(),
                                )));
                                break Err(diagnostic);
                            }
                            self.diagnostics.push(diagnostic);
                            self.synchronize_body_item(body_depth, start);
                            items.push(BlockItem::Unavailable(Span::new(
                                self.tokens[start].span.start,
                                self.body_consumed_end(),
                            )));
                        }
                    }
                }
            }
        };
        let span = match &result {
            Ok(span) => *span,
            Err(_) => Span::new(open.span.start, self.peek().span.start),
        };
        let recovered = if result.is_err() || !self.fragment_is_complete(checkpoint) {
            Some(
                items
                    .into_iter()
                    .map(|item| match item {
                        BlockItem::Statement {
                            index,
                            complete: true,
                        } => RecoveredStatement::Complete(Box::new(statements[index].clone())),
                        BlockItem::Statement {
                            index,
                            complete: false,
                        } => RecoveredStatement::Unavailable(statements[index].span),
                        BlockItem::Unavailable(span) => RecoveredStatement::Unavailable(span),
                    })
                    .collect(),
            )
        } else {
            None
        };
        self.finish_body_block(span, result.is_ok(), recovered);
        result.map(|span| Block { statements, span })
    }

    fn body_consumed_end(&self) -> u32 {
        self.pos
            .checked_sub(1)
            .map_or(0, |index| self.tokens[index].span.end)
    }
}
